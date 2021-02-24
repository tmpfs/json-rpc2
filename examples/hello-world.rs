use json_rpc2::*;
use serde_json::Value;

struct ServiceHandler;
impl Service for ServiceHandler {
    type Data = ();
    fn handle(
        &self,
        request: &mut Request,
        _ctx: &Self::Data,
    ) -> Result<Option<Response>> {
        let mut response = None;
        if request.matches("hello") {
            let params: String = request.deserialize()?;
            let message = format!("Hello, {}!", params);
            response = Some((request, Value::String(message)).into());
        }
        Ok(response)
    }
}

fn main() -> Result<()> {
    let service: Box<dyn Service<Data = ()>> = Box::new(ServiceHandler {});
    let mut request =
        Request::new_reply("hello", Some(Value::String("world".to_string())));
    let server = Server::new(vec![&service]);
    let response = server.serve(&mut request, &());
    println!("{:?}", response.as_ref().unwrap().result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.unwrap().into()
    );
    Ok(())
}
