Feature: Unified Credential Provider
  As a user with private servers
  I want the engine to automatically use .netrc and cookie credentials
  So that I don't have to embed secrets in URLs.

  @ADR-0014
  Scenario: Authenticated HTTP download via .netrc
    Given an HTTP mirror requiring Basic Auth
    And a .netrc file with:
      | machine   | login  | password |
      | 127.0.0.1 | myuser | mypass   |
    When I add the authenticated task
    Then the "HttpWorker" should successfully authenticate and download the file

  Scenario: Authenticated HTTP download via Cookies
    Given an HTTP mirror requiring a "session_id" cookie
    And a cookie file for "127.0.0.1" with "session_id=secret-token"
    When I add the authenticated task
    Then the "HttpWorker" should successfully send the cookie and download the file

  Scenario: Authenticated FTP download via .netrc
    Given an FTP mirror requiring login
    And a .netrc file with:
      | machine   | login   | password |
      | 127.0.0.1 | ftpuser | ftppass  |
    When I add the authenticated task
    Then the "FtpWorker" should successfully login and download the file

  Scenario: Multiple entries in .netrc
    Given an HTTP mirror requiring Basic Auth for "userA:passA"
    And an HTTP mirror requiring Basic Auth for "userB:passB"
    And a .netrc file with:
      | machine   | login | password |
      | 127.0.0.1 | userA | passA    |
      | localhost | userB | passB    |
    When I add a task for "http://127.0.0.1/file"
    Then the download for "127.0.0.1" should succeed
    When I add a task for "http://localhost/file"
    Then the download for "localhost" should succeed
