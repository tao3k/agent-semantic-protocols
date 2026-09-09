Expected route:

- metadata route: main-agent direct inventory
- final evidence route: main-agent direct fetch via `--content` or `--code`
- subagent dispatch count: 0 unless fan-out or synthesis is required
- duplicate model read count: 0
- forbidden routes for this scenario: repeated Search Playbook and raw-read
