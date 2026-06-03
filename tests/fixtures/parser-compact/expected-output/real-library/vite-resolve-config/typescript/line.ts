async function resolveConfig
  assign config
  setupRollupOptionCompat
  if config.ssr
  assign configFileDependencies
  assign mode
  assign isNodeEnvSet
  assign packageCache
  if !isNodeEnvSet
    process.env.NODE_ENV = defaultNodeEnv
  assign configEnv
  assign { configFile }
  if configFile !== false
    assign loadResult
      await
        loadConfigFromFile
    if loadResult
      config = mergeConfig(loadResult.config, config)
      configFile = loadResult.path
      configFileDependencies = loadResult.dependencies
  mode = inlineConfig.mode || config.mode || mode
  configEnv.mode = mode
