use super::prelude::*;

pub trait ResponseHandler {
    fn handle_response(&self, response: Response) -> Pin<Box<dyn Future<Output = Result<()>>>>;
}
