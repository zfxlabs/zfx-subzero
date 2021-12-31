use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio_serde::Framed;
use tokio_serde::formats::*;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

#[derive(Debug)]
pub enum Error<'a, I, O> where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
{
    IO(std::io::Error),
    ReadError(<Reader<'a, I, O> as futures::TryStream>::Error),
    WriteError(<Writer<'a, I, O> as futures::Sink<I>>::Error),
}

pub type Reader<'a, I, O> = Framed<FramedRead<ReadHalf<'a>, LengthDelimitedCodec>, O, I, Bincode<O, I>>;

pub type Writer<'a, I, O> = Framed<FramedWrite<WriteHalf<'a>, LengthDelimitedCodec>, O, I, Bincode<O, I>>;

pub struct Receiver<'a, I, O> {
    reader: Reader<'a, I, O>,
}

impl<'a, I, O> Receiver<'a, I, O> where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
Reader<'a, I, O> : TryStream<Ok=O> + Unpin,
{
    pub async fn recv(&mut self) -> Result<Option<O>, Error<'a, I, O>> {
        Ok(self.reader.try_next().await.map_err(Error::ReadError)?)
    }
}

pub struct Sender<'a, I, O> {
    writer: Writer<'a, I, O>,
}

impl<'a, I, O> Sender<'a, I, O> where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
Writer<'a, I, O> : Sink<I> + Unpin
{
    pub async fn send(&mut self, item: I) -> Result<(), Error<'a, I, O>> {
        Ok(self.writer.send(item).await.map_err(Error::WriteError)?)
    }
}

pub struct Channel<I, O> {
    socket: TcpStream,
    ghost: std::marker::PhantomData<(I, O)>,
}

impl<'a, I, O> Channel<I, O> where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
{
    pub async fn connect(address: &SocketAddr) -> Result<Channel<I, O>, Error<'a, I, O>>
    {
        let socket = TcpStream::connect(&address).await.map_err(Error::IO)?;
        Ok(Channel{ socket, ghost: Default::default() })
    }

    pub async fn accept(listener: &TcpListener) -> Result<Channel<I, O>, Error<'a, I, O>>
    {
        let (socket, _) = listener.accept().await.map_err(Error::IO)?;
        Ok(Channel{ socket, ghost: Default::default() })
    }

    pub fn split(&mut self) -> (Sender<'_, I, O>, Receiver<'_, I, O>) {
        let (reader, writer) = self.socket.split();

        let reader: FramedRead<ReadHalf, LengthDelimitedCodec> =
	    FramedRead::new(reader, LengthDelimitedCodec::new());
        let reader = Framed::new(reader, Bincode::default());

        let writer: FramedWrite<WriteHalf, LengthDelimitedCodec> =
	    FramedWrite::new(writer, LengthDelimitedCodec::new());
        let writer = Framed::new(writer, Bincode::default());

        (Sender{ writer }, Receiver{ reader })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use std::net::SocketAddr;
    use std::str::FromStr;

    #[actix_rt::test]
    async fn asymmetric_send_recv() {
        use crate::channel::Channel;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        pub struct Request(String);
        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        pub struct Response(String);

        let handle_1 = tokio::spawn(async {
            let address: SocketAddr = "127.0.0.1:20000".parse()
                .expect("failed to construct address");
	    let listener = TcpListener::bind(&address).await.unwrap();
            let mut channel = Channel::accept(&listener).await
                .expect("failed to accept connection");

            let (mut sender, mut receiver) = channel.split();

            // Send message:
            sender.send(Request(String::from("123"))).await.unwrap();

            // Receive message:
            let msg = receiver.recv().await.unwrap();
            assert_eq!(msg, Some(Response(String::from("321"))));

            // Send message:
            sender.send(Request(String::from("456"))).await.unwrap();

            // Receive message:
            let msg = receiver.recv().await.unwrap();
            assert_eq!(msg, Some(Response(String::from("654"))));
        });

        let handle_2 = tokio::spawn(async {
            let address: SocketAddr = "127.0.0.1:20000".parse()
                .expect("failed to construct address");
            let mut channel: Channel<Response, Request> = Channel::connect(&address).await
                .expect("failed to accept connection");

            let (mut sender, mut receiver) = channel.split();

            // Receive message:
            let msg = receiver.recv().await.unwrap();
            assert_eq!(msg, Some(Request(String::from("123"))));

            // Send message:
            sender.send(Response(String::from("321"))).await.unwrap();

            // Receive message:
            let msg = receiver.recv().await.unwrap();
            assert_eq!(msg, Some(Request(String::from("456"))));

            // Send message:
            sender.send(Response(String::from("654"))).await.unwrap();
        });

        handle_2.await.unwrap();
        handle_1.await.unwrap();
    }
}
