use super::prelude::*;

pub trait ResponseHandler<T> {
    fn handle_response(&self, response: T) -> Pin<Box<dyn Future<Output = Result<()>>>>;
}
