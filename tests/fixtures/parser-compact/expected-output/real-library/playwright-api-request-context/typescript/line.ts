class APIRequestContext
  field _request?: APIRequest
  field readonly tracing: Tracing
  field _closeReason: string | undefined
  field _timeoutSettings: TimeoutSettings
  method from
    return _object
  constructor
    call
    this.tracing = Tracing.from(initializer.tracing)
    this._timeoutSettings = new TimeoutSettings(this._platform)
  async method [Symbol.asyncDispose]
    await
      dispose
  async method dispose
    field reason?: string
    this._closeReason = options.reason
      runBeforeCloseRequestContext
      _exportAllHars
    try
