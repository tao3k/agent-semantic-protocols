function buildCreateSlice
  assign cAT
  return function createSlice
    assign { name, reducerPath = name as unknown as ReducerPath }
    if !name
      throw new Error
    if typeof process !== 'undefined' && process.env.NODE_ENV === 'development'
      if options.initialState === undefined
    assign reducers
    assign reducerNames
    assign context
    assign contextMethods
      method addCase
        assign type
        if !type
        if type in context.sliceCaseReducersByType
        context.sliceCaseReducersByType[type] = reducer
        return contextMethods
      method addMatcher
        push
      method exposeAction
