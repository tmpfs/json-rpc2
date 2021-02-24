use json_rpc2::*;
use serde_json::Value;

#[derive(Debug)]
struct ServiceData {
    pub message: String,
}

struct ServiceHandler;
impl Service for ServiceHandler {
    type Data = ServiceData;
    fn handle(
        &self,
        request: &mut Request,
        ctx: &Self::Data,
    ) -> Result<Option<Response>> {
        let mut response = None;
        if request.matches("hello") {
            let message = format!("Hello, {}!", &ctx.message);
            response = Some((request, Value::String(message)).into());
        }
        Ok(response)
    }
}

fn main() -> Result<()> {
    let service: Box<dyn Service<Data = ServiceData>> = Box::new(ServiceHandler {});
    let mut request = Request::new("hello", None);
    let server = Server::new(vec![&service]);
    let data = ServiceData { message: "world".to_string() };
    let response = server.serve(&mut request, &data);
    println!("{:?}", response.as_ref().unwrap().result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.unwrap().into()
    );
    Ok(())
}
