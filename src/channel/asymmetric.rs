use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{ReadHalf, WriteHalf};
//use tokio::net::TcpStream;
use tokio_serde::formats::*;
use tokio_serde::Framed;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::tls::connection_stream::ConnectionStream;

#[derive(Debug)]
pub enum Error<I, O>
where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
{
    IO(std::io::Error),
    ReadError(<Reader<I, O> as futures::TryStream>::Error),
    WriteError(<Writer<I, O> as futures::Sink<I>>::Error),
}

pub type Reader<I, O> =
    Framed<FramedRead<ReadHalf<ConnectionStream>, LengthDelimitedCodec>, O, I, Bincode<O, I>>;

pub type Writer<I, O> =
    Framed<FramedWrite<WriteHalf<ConnectionStream>, LengthDelimitedCodec>, O, I, Bincode<O, I>>;

pub struct Receiver<I, O> {
    reader: Reader<I, O>,
}

impl<I, O> Receiver<I, O>
where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
    Reader<I, O>: TryStream<Ok = O> + Unpin,
{
    pub async fn recv(&mut self) -> Result<Option<O>, Error<I, O>> {
        Ok(self.reader.try_next().await.map_err(Error::ReadError)?)
    }
}

pub struct Sender<I, O> {
    writer: Writer<I, O>,
}

impl<I, O> Sender<I, O>
where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
    Writer<I, O>: Sink<I> + Unpin,
{
    pub async fn send(&mut self, item: I) -> Result<(), Error<I, O>> {
        Ok(self.writer.send(item).await.map_err(Error::WriteError)?)
    }
}

pub struct Channel<I, O> {
    socket: ConnectionStream,
    ghost: std::marker::PhantomData<(I, O)>,
}

impl<I, O> Channel<I, O>
where
    I: for<'de> Deserialize<'de> + Serialize,
    O: for<'de> Deserialize<'de> + Serialize,
{
//    pub async fn connect(address: &SocketAddr) -> Result<Channel<I, O>, Error<I, O>> {
//        Ok(Channel { socket, ghost: Default::default() })
//    }

    pub fn wrap(socket: ConnectionStream) -> Result<Channel<I, O>, Error<I, O>> {
        Ok(Channel { socket, ghost: Default::default() })
    }

    pub fn split(&mut self) -> (Sender<I, O>, Receiver<I, O>) {
        let (reader, writer) = tokio::io::split(self.socket);

        let reader: FramedRead<ReadHalf<_>, LengthDelimitedCodec> =
            FramedRead::new(reader, LengthDelimitedCodec::new());
        let reader = Framed::new(reader, Bincode::default());

        let writer: FramedWrite<WriteHalf<_>, LengthDelimitedCodec> =
            FramedWrite::new(writer, LengthDelimitedCodec::new());
        let writer = Framed::new(writer, Bincode::default());

        (Sender { writer }, Receiver { reader })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;
    use tokio::net::TcpListener;

    #[actix_rt::test]
    async fn asymmetric_send_recv() {
        use crate::channel::Channel;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        pub struct Request(String);
        #[derive(Debug, PartialEq, Deserialize, Serialize)]
        pub struct Response(String);

        let handle_1 = tokio::spawn(async {
            let address: SocketAddr =
                "127.0.0.1:20000".parse().expect("failed to construct address");
            let listener = TcpListener::bind(&address).await.unwrap();
            let (socket, address) = listener.accept().await.unwrap();
            let mut channel = Channel::wrap(socket).expect("failed to accept connection");

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
            let address: SocketAddr =
                "127.0.0.1:20000".parse().expect("failed to construct address");
            let mut channel: Channel<Response, Request> =
                Channel::connect(&address).await.expect("failed to accept connection");

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
