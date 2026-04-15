# Add an integration test with Playwright to test password changes.

So the goal is to write an integration test with Playwright that
1. start the Terrazzo server
    - with a temp config file that contains
      ```
      [server.config_file_poll_strategy]
      fixed = "1s"
      ```
    - initially the temp config file can be empty, terrazzo will initialize it with defaults
    - use the //terminal:terminal-integration-test-(debug|release) target
2. at first there is no password so user gets logged-in immediately. For example you can see the 'div[class*="add-tab-icon-"] img' (aka getAddTabButton in terminal/tests/integration-test-terminal.spec.mjs)
3. then call `terrazzo-terminal --config-file $CONFIG_FILE_PATH -a set-password`, enter a random password. 
4. then reload the page &rarr; now user does not get logged-in immediately. Enter the password. then confirm the user is logged-in and the add-tab-icon- shows up

Questions:
A. Is it possible to call a CLI from within the Playwright test
