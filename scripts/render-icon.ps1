# Renders assets/icon.svg's geometry with GDI+ into assets/icon.ico (16 to 256 px, PNG entries)
# and assets/icon-256.png. The drawing below mirrors the SVG; change both together.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$assets = Join-Path $PSScriptRoot '..\assets'

function Draw([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $s = $size / 1024.0
    $g.ScaleTransform($s, $s)

    $tile = New-Object System.Drawing.Drawing2D.GraphicsPath
    $r = 431
    $tile.AddArc(32, 32, $r, $r, 180, 90); $tile.AddArc(992 - $r, 32, $r, $r, 270, 90)
    $tile.AddArc(992 - $r, 992 - $r, $r, $r, 0, 90); $tile.AddArc(32, 992 - $r, $r, $r, 90, 90)
    $tile.CloseFigure()
    $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush (New-Object System.Drawing.PointF 0, 32), (New-Object System.Drawing.PointF 0, 992), ([System.Drawing.Color]::FromArgb(255, 0x13, 0x12, 0x17)), ([System.Drawing.Color]::Black)
    $g.FillPath($bg, $tile)

    $white = [System.Drawing.Color]::FromArgb(255, 0xfc, 0xfc, 0xfc)
    # Thicker strokes at small sizes keep the outline readable.
    $boost = if ($size -le 32) { 1.6 } elseif ($size -le 64) { 1.25 } else { 1.0 }
    $pen = New-Object System.Drawing.Pen $white, (40 * $boost)
    $pen.LineJoin = 'Round'
    $body = New-Object System.Drawing.Drawing2D.GraphicsPath
    $body.AddLine(300, 380, 724, 380)
    $body.AddBezier(724, 380, 812, 380, 858, 436, 876, 528)
    $body.AddLine(876, 528, 912, 700)
    $body.AddBezier(912, 700, 926, 772, 866, 808, 820, 762)
    $body.AddLine(820, 762, 738, 680)
    $body.AddLine(738, 680, 286, 680)
    $body.AddLine(286, 680, 204, 762)
    $body.AddBezier(204, 762, 158, 808, 98, 772, 112, 700)
    $body.AddLine(112, 700, 148, 528)
    $body.AddBezier(148, 528, 166, 436, 212, 380, 300, 380)
    $body.CloseFigure()
    $g.DrawPath($pen, $body)

    $cross = New-Object System.Drawing.Pen $white, (38 * $boost)
    $cross.StartCap = 'Round'; $cross.EndCap = 'Round'
    $g.DrawLine($cross, 326, 470, 326, 590)
    $g.DrawLine($cross, 266, 530, 386, 530)

    $dot = 30 * [Math]::Min($boost, 1.3)
    foreach ($d in @(@(698, 468, '2fbf71'), @(758, 530, 'e60000'), @(698, 592, '4c7dff'), @(638, 530, 'ff5fa2'))) {
        $c = [System.Drawing.ColorTranslator]::FromHtml('#' + $d[2])
        $g.FillEllipse((New-Object System.Drawing.SolidBrush $c), $d[0] - $dot, $d[1] - $dot, 2 * $dot, 2 * $dot)
    }
    $g.Dispose()
    $bmp
}

$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$pngs = foreach ($n in $sizes) {
    $bmp = Draw $n
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($n -eq 256) { $bmp.Save((Join-Path $assets 'icon-256.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $bmp.Dispose()
    , $ms.ToArray()
}

# ICO: header, one directory entry per size, then the PNG images.
$out = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $out
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $n = $sizes[$i]; $b = if ($n -ge 256) { 0 } else { $n }
    $w.Write([byte]$b); $w.Write([byte]$b); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$pngs[$i].Length); $w.Write([uint32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $w.Write($p) }
[System.IO.File]::WriteAllBytes((Join-Path $assets 'icon.ico'), $out.ToArray())
Write-Host "Wrote assets/icon.ico and assets/icon-256.png"
