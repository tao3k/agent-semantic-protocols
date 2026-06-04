class Session
  field headers: CaseInsensitiveDict[str]
  field auth: _t.AuthType
  field proxies: dict[str, str]
  field hooks: dict[str, list[_t.HookType]]
  field params: MutableMapping[str, Any]
  field stream: bool
  field verify: _t.VerifyType
  field cert: _t.CertType
  field max_redirects: int
  field trust_env: bool
  field cookies: RequestsCookieJar
  field adapters: MutableMapping[str, BaseAdapter]
  field __attrs__: list[str]
  def __init__
    assign self.headers
    assign self.auth
    assign self.proxies
    assign self.hooks
    assign self.params
    assign self.stream
    assign self.verify
    assign self.cert
    assign self.max_redirects
