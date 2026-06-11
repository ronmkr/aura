Feature: Cloud Storage Support
  As an Aura engine user
  I want to download files from S3-compatible APIs, Google Drive, and OneDrive
  So that I can retrieve modern cloud-hosted datasets.

  @ADR-0013 @s3
  Scenario: Download a file from S3-compatible storage
    Given a mock S3 bucket "test-bucket" with key "data.bin" containing "S3 cloud storage data"
    When I add a task for S3 URL "s3://test-bucket/data.bin"
    Then the download should complete successfully
    And the downloaded file should contain "S3 cloud storage data"

  @ADR-0013 @gdrive
  Scenario: Download a file from Google Drive
    Given a mock Google Drive file "gdrive-file-123" containing "GDrive personal cloud data"
    When I add a task for GDrive URL "gdrive://gdrive-file-123"
    Then the download should complete successfully
    And the downloaded file should contain "GDrive personal cloud data"

  @ADR-0013 @gdrive
  Scenario: Download a file from OneDrive
    Given a mock OneDrive item "onedrive-item-456" containing "OneDrive corporate cloud data"
    When I add a task for OneDrive URL "onedrive://onedrive-item-456"
    Then the download should complete successfully
    And the downloaded file should contain "OneDrive corporate cloud data"

