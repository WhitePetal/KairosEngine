# WS test client — connects to harness WS server and triggers a TOML test
[CmdletBinding()]
param(
    [string]$TestFile = "tests/runtime/smoke_test.toml"
)

$url = "ws://127.0.0.1:9999"
$ws = New-Object System.Net.WebSockets.ClientWebSocket
$ct = New-Object System.Threading.CancellationToken

Write-Host "Connecting to $url ..."
$ws.ConnectAsync([Uri]$url, $ct).Wait()

# Send run_test command
$cmd = @{ cmd = "run_test"; file = $TestFile } | ConvertTo-Json -Compress
Write-Host "Sending: $cmd"
$bytes = [System.Text.Encoding]::UTF8.GetBytes($cmd)
$seg = New-Object System.ArraySegment[byte] -ArgumentList @(,$bytes)
$ws.SendAsync($seg, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $ct).Wait()

# Receive response
$buffer = New-Object byte[] 4096
$rseg = New-Object System.ArraySegment[byte] -ArgumentList @(,$buffer)
$result = $ws.ReceiveAsync($rseg, $ct).Result
$response = [System.Text.Encoding]::UTF8.GetString($buffer, 0, $result.Count)
Write-Host "Response: $response"

$ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "", $ct).Wait()
Write-Host "Done."
