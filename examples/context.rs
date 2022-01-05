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
        request: &Request,
        ctx: &Self::Data,
    ) -> Result<Option<Response>> {
        let response = match request.method() {
            "hello" => {
                let message = format!("Hello, {}!", &ctx.message);
                Some((request, Value::String(message)).into())
            }
            _ => None,
        };
        Ok(response)
    }
}

fn main() -> Result<()> {
    let service: Box<dyn Service<Data = ServiceData>> =
        Box::new(ServiceHandler {});
    let request = Request::new_reply("hello", None);
    let server = Server::new(vec![&service]);
    let data = ServiceData {
        message: "world".to_string(),
    };
    let response = server.serve(&request, &data);
    println!("{:?}", response.as_ref().unwrap().result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.unwrap().into()
    );
    Ok(())
}
