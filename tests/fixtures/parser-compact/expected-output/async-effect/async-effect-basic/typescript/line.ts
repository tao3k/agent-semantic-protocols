async function loadUser
  assign response
    await
      fetch
  if !response.ok
    throw new Error
  assign body
      text
  return body
