use json_rpc2::*;
use serde_json::Value;

struct ServiceHandler;
impl<T> Service<T> for ServiceHandler {
    fn handle(&self, request: &mut Request, _context: &Context<T>) -> Result<Option<Response>> {
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
    let service: Box<dyn Service<()>> = Box::new(ServiceHandler {});
    let mut request = Request::new("hello", Some(Value::String("world".to_string())));
    let services = vec![&service];
    let response = serve(&services, &mut request);
    println!("{:?}", response.result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.into()
    );
    Ok(())
}
