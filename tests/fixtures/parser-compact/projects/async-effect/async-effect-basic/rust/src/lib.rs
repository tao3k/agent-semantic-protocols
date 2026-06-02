pub struct Response {
    pub ok: bool,
    pub body: String,
}

impl Response {
    pub async fn text(self) -> String {
        self.body
    }
}

pub struct Client;

impl Client {
    pub async fn fetch(&self, user_id: u64) -> Result<Response, String> {
        if user_id == 0 {
            return Err("missing".to_string());
        }
        Ok(Response {
            ok: true,
            body: format!("user:{user_id}"),
        })
    }
}

pub async fn load_user(client: &Client, user_id: u64) -> Result<String, String> {
    let response = client.fetch(user_id).await?;
    if !response.ok {
        return Err("missing".to_string());
    }
    let body = response.text().await;
    Ok(body)
}
