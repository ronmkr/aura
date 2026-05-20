Feature: Unified Credential Provider
  As a user with private servers
  I want the engine to automatically use .netrc and cookie credentials
  So that I don't have to embed secrets in URLs.

  @ADR-0014
  Scenario: Authenticated HTTP download via .netrc
    Given an HTTP mirror at "http://private.example.com/file" requiring Basic Auth
    And a .netrc file with:
      | machine             | login  | password |
      | private.example.com | myuser | mypass   |
    When I add the authenticated task
    Then the "HttpWorker" should successfully authenticate and download the file

  Scenario: Authenticated HTTP download via Cookies
    Given an HTTP mirror at "http://cookies.example.com/file" requiring a "session_id" cookie
    And a cookie file for "cookies.example.com" with "session_id=secret-token"
    When I add the authenticated task
    Then the "HttpWorker" should successfully send the cookie and download the file

  Scenario: Authenticated FTP download via .netrc
    Given an FTP mirror at "ftp://ftp.private.com/file" requiring login
    And a .netrc file with:
      | machine         | login   | password |
      | ftp.private.com | ftpuser | ftppass  |
    When I add the authenticated task
    Then the "FtpWorker" should successfully login and download the file
