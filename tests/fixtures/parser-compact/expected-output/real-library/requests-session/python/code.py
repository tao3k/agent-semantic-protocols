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
    assign self.trust_env
    assign self.cookies
    assign self.adapters
    call mount:https://
    call mount:http://
  def __enter__
    return self
  def __exit__
    call close
  def prepare_request
    assign url
    assign method
    assign cookies
    if not isinstance(cookies, cookielib.CookieJar)
      assign cookies
    assign merged_cookies
    assign auth
    if self.trust_env and (not auth) and (not self.auth)
      assign auth
    assign p
    call prepare
    return p
  def request
    if isinstance
      assign url
    assign req
    assign prep
    assign proxies
    assign settings
    assign send_kwargs
    call update
    assign resp
    return resp
  def get
    call setdefault:allow_redirects
    return request:GET
  def options
    call setdefault:allow_redirects
    return request:OPTIONS
  def head
    call setdefault:allow_redirects
    return request:HEAD
  def post
    return request:POST
  def put
    return request:PUT
  def patch
    return request:PATCH
  def delete
    return request:DELETE
  def send
    call setdefault:stream
    call setdefault:verify
    call setdefault:cert
    if 'proxies' not in kwargs
      assign kwargs['proxies']
    if isinstance
      raise ValueError:You can only send PreparedRequests.
    assign allow_redirects
    assign stream
    assign hooks
    assign adapter
    assign start
    assign r
    assign elapsed
    assign r.elapsed
    assign r
    if r.history
      for resp in r.history
        call extract_cookies_to_jar
    call extract_cookies_to_jar
    if allow_redirects
      assign gen
      assign history
      assign history
    if history
      call insert
      assign r
      assign r.history
    if not allow_redirects
      try
        assign r._next
        except StopIteration
    if not stream
    return r
  def merge_environment_settings
    if self.trust_env
      assign no_proxy
      assign env_proxies
      if proxies is not None
        for (k, v) in items
          call setdefault
      if verify is True or verify is None
        assign verify
    assign proxies
    assign stream
    assign verify
    assign cert
    return {'proxies': proxies, 'stream': stream, 'verify': verify, 'cert': cert}
  def get_adapter
    for (prefix, adapter) in items
      if startswith
        return adapter
    raise InvalidSchema
  def close
    for v in values
      call close
  def mount
    assign self.adapters[prefix]
    assign keys_to_move
    for key in keys_to_move
      assign self.adapters[key]
  def __getstate__
    assign state
    return state
  def __setstate__
    for (attr, value) in items
      call setattr
