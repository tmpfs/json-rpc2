use json_rpc2::{Request, Response, Result, futures::*};
use serde_json::Value;
use async_trait::async_trait;

struct ServiceHandler;

#[async_trait]
impl Service for ServiceHandler {
    async fn handle(&self, req: &mut Request) -> Result<Option<Response>> {
        let mut response = None;
        if req.matches("hello") {
            let params: String = req.deserialize()?;
            let message = format!("Hello, {}!", params);
            response = Some((req, Value::String(message)).into());
        }
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service: Box<dyn Service> = Box::new(ServiceHandler {});
    let mut request = Request::new("hello", Some(Value::String("world".to_string())));
    let services = vec![&service];
    let response = serve(&services, &mut request).await;
    println!("{:?}", response.result());
    assert_eq!(
        Some(Value::String("Hello, world!".to_string())),
        response.into()
    );
    Ok(())
}
