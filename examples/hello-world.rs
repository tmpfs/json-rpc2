use json_rpc2::*;
use serde_json::Value;

struct ServiceHandler;
impl Service for ServiceHandler {
    fn handle(&self, req: &mut Request) -> Result<Option<Response>> {
        let mut response = None;
        if req.matches("hello") {
            let params: String = req.into_params()?;
            let message = format!("Hello, {}!", params);
            response = Some((req, Value::String(message)).into());
        }
        Ok(response)
    }
}

fn main() -> Result<()> {
    let service: Box<dyn Service> = Box::new(ServiceHandler {});
    let mut request = Request::new("hello", Some(Value::String("world".to_string())));
    let services = vec![&service];
    let response = serve(&services, &mut request);
    println!("{:?}", response.result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.into_result()
    );
    Ok(())
}
