# Generates assets/icon.png (256x256) and a multi-resolution assets/icon.ico
# for OxideDisk Analyzer. Placeholder art: a rounded-square gradient tile (oxide
# orange -> deep slate) with a stylized disk ring and an "O" glyph.
#
# Run:  powershell -ExecutionPolicy Bypass -File assets/generate_icon.ps1
Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

function New-IconBitmap([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    $s = [float]$size
    $pad = [float]($s * 0.06)
    $radius = [float]($s * 0.22)
    $wh = [float]($s - (2 * $pad))
    $rect = New-Object System.Drawing.RectangleF($pad, $pad, $wh, $wh)

    # Rounded-square path.
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = [float]($radius * 2)
    $path.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $path.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $path.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()

    # Diagonal gradient fill: oxide orange -> deep slate.
    $c1 = [System.Drawing.Color]::FromArgb(255, 230, 126, 34)   # oxide orange
    $c2 = [System.Drawing.Color]::FromArgb(255, 28, 36, 48)     # deep slate
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, $c1, $c2, 55.0)
    $g.FillPath($brush, $path)

    # Disk ring (stylized): two concentric circles, light stroke.
    $cx = $s / 2.0
    $cy = $s / 2.0
    $rOuter = $s * 0.30
    $rInner = $s * 0.085
    $penW = [Math]::Max(1.0, $s * 0.035)
    $ring = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(235, 245, 247, 250), $penW)
    $g.DrawEllipse($ring, $cx - $rOuter, $cy - $rOuter, $rOuter*2, $rOuter*2)
    $hub = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(235, 245, 247, 250))
    $g.FillEllipse($hub, $cx - $rInner, $cy - $rInner, $rInner*2, $rInner*2)

    $g.Flush()
    $g.Dispose()
    $brush.Dispose(); $ring.Dispose(); $hub.Dispose(); $path.Dispose()
    return $bmp
}

function Get-PngBytes($bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $ms.ToArray()
    $ms.Dispose()
    return ,$bytes
}

# --- 256x256 master PNG ---
$master = New-IconBitmap 256
$pngPath = Join-Path $here 'icon.png'
$master.Save($pngPath, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "wrote $pngPath"

# --- multi-resolution ICO (PNG-compressed entries; Vista+) ---
$sizes = 16, 32, 48, 64, 128, 256
$images = foreach ($sz in $sizes) {
    $b = New-IconBitmap $sz
    $png = Get-PngBytes $b
    $b.Dispose()
    [pscustomobject]@{ Size = $sz; Bytes = $png }
}
$master.Dispose()

$icoPath = Join-Path $here 'icon.ico'
$fs = New-Object System.IO.FileStream($icoPath, [System.IO.FileMode]::Create)
$bw = New-Object System.IO.BinaryWriter($fs)

# ICONDIR
$bw.Write([UInt16]0)                     # reserved
$bw.Write([UInt16]1)                     # type = icon
$bw.Write([UInt16]$images.Count)         # count

# ICONDIRENTRY[] — image data starts after dir.
$offset = 6 + (16 * $images.Count)
foreach ($img in $images) {
    $w = if ($img.Size -ge 256) { 0 } else { $img.Size }
    $bw.Write([Byte]$w)                   # width  (0 => 256)
    $bw.Write([Byte]$w)                   # height (0 => 256)
    $bw.Write([Byte]0)                    # palette count
    $bw.Write([Byte]0)                    # reserved
    $bw.Write([UInt16]1)                  # color planes
    $bw.Write([UInt16]32)                 # bits per pixel
    $bw.Write([UInt32]$img.Bytes.Length)  # bytes in resource
    $bw.Write([UInt32]$offset)            # offset
    $offset += $img.Bytes.Length
}
foreach ($img in $images) { $bw.Write($img.Bytes) }

$bw.Flush(); $bw.Close(); $fs.Close()
Write-Host "wrote $icoPath ($($images.Count) sizes)"
