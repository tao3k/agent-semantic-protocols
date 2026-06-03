class APIRouter
  def __init__
    if lifespan is None
      assign lifespan_context
      if isasyncgenfunction
        assign lifespan_context
        if isgeneratorfunction
          assign lifespan_context
          assign lifespan_context
    assign self.lifespan_context
    call __init__
    if prefix
    assign self.on_startup
    assign self.on_shutdown
    assign self.prefix
    assign self.tags
    assign self.dependencies
    assign self.deprecated
    assign self.include_in_schema
    assign self.responses
    assign self.callbacks
    assign self.dependency_overrides_provider
    assign self.route_class
    assign self.default_response_class
    assign self.generate_unique_id_function
    assign self.strict_content_type
  def route
    def decorator
      call add_route
      return func
    return decorator
  def add_api_route
    assign route_class
    assign responses
    assign combined_responses
    assign current_response_class
    assign current_tags
    if tags
      call extend
    assign current_dependencies
    if dependencies
      call extend
    assign current_callbacks
    if callbacks
      call extend
    assign current_generate_unique_id
    assign route
    call append
  def api_route
    def decorator
      call add_api_route
      return func
    return decorator
  def add_api_websocket_route
    assign current_dependencies
    if dependencies
      call extend
    assign route
    call append
  def websocket
    def decorator
      call add_api_websocket_route
      return func
    return decorator
  def websocket_route
    def decorator
      call add_websocket_route
      return func
    return decorator
  def include_router
    if prefix
      for r in router.routes
        assign path
        assign name
        if path is not None and (not path)
          raise FastAPIError
    if responses is None
      assign responses
    for route in router.routes
      if isinstance
        assign combined_responses
        assign use_response_class
        assign current_tags
        if tags
          call extend
        if route.tags
          call extend
        assign current_dependencies
        if dependencies
          call extend
        if route.dependencies
          call extend
        assign current_callbacks
        if callbacks
          call extend
        if route.callbacks
          call extend
        assign current_generate_unique_id
        call add_api_route
        if isinstance
          assign methods
          call add_route
          if isinstance
            assign current_dependencies
            if dependencies
              call extend
            if route.dependencies
              call extend
            call add_api_websocket_route
            if isinstance
              call add_websocket_route
    for handler in router.on_startup
      call add_event_handler:startup
    for handler in router.on_shutdown
      call add_event_handler:shutdown
    assign self.lifespan_context
  def get
    return api_route
  def put
    return api_route
  def post
    return api_route
  def delete
    return api_route
  def options
    return api_route
  def head
    return api_route
  def patch
    return api_route
  def trace
    return api_route
  async def _startup
    for handler in self.on_startup
      if is_async_callable
        await handler
          await handler
        call handler
  async def _shutdown
    for handler in self.on_shutdown
      if is_async_callable
        await handler
          await handler
        call handler
  def add_event_handler
    if event_type == 'startup'
      call append
      call append
  def on_event
    @deprecated:
        on_event is deprecated, use lifespan event handlers instead.

        Read more about it in the
        [FastAPI docs for Lifespan Events](https://fastapi.tiangolo.com/advanced/events/).
    def decorator
      call add_event_handler
      return func
    return decorator
