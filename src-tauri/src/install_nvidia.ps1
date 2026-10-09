$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$dir = Join-Path $env:TEMP 'better-mic-nvidia'
$failed = $false
try {
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory $dir | Out-Null
    $exe = Join-Path $dir 'nvidia-afx-installer.exe'

    Add-Type -AssemblyName System.Net.Http
    $client = New-Object System.Net.Http.HttpClient
    $client.Timeout = [TimeSpan]::FromMinutes(60)
    $resp = $client.GetAsync('{URL}', [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
    [void]$resp.EnsureSuccessStatusCode()
    $total = [long]$resp.Content.Headers.ContentLength
    $in = $resp.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
    $out = [IO.File]::Create($exe)
    try {
        $buf = New-Object byte[] 1048576
        $done = 0L
        $last = 0L
        while (($n = $in.Read($buf, 0, $buf.Length)) -gt 0) {
            $out.Write($buf, 0, $n)
            $done += $n
            if ($done - $last -ge 4194304) { $last = $done; 'PROGRESS {0} {1}' -f $done, $total }
        }
    } finally {
        $out.Dispose()
        $in.Dispose()
    }
    'PROGRESS {0} {1}' -f $done, $total

    'STAGE verify'
    $sig = Get-AuthenticodeSignature $exe
    if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'NVIDIA') {
        throw ('The downloaded installer is not signed by NVIDIA ({0}).' -f $sig.Status)
    }

    'STAGE install'
    Start-Process $exe -Verb RunAs -Wait
} catch {
    $e = $_.Exception
    while ($e.InnerException) { $e = $e.InnerException }
    'ERROR ' + ($e.Message -replace '\s+', ' ')
    $failed = $true
} finally {
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { exit 1 }
