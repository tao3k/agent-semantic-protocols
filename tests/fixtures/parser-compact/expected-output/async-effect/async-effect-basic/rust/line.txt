pub async fn load_user(client: &Client, user_id: u64) -> Result<String, String>
let response = fetch.await?
if ! response.ok
return Err("missing".to_string())
let body = text.await
call Ok
