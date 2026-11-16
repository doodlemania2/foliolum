<#
.SYNOPSIS
  Makes Foliolum's Phase 1 gate corpus: bulletin-style Microsoft Publisher files, each with Publisher's own
  PDF of it as the reference rendering.

.DESCRIPTION
  Run it at the console of a Windows machine with Publisher installed (Office 2021 tested target). It drives
  Publisher through COM, so it needs an interactive desktop session. It needs no admin rights, installs
  nothing and uses no network.

  Everything goes to %USERPROFILE%\foliolum-corpus\ :
    pictures\            invented pictures the files use (drawn here, no downloads)
    pub\NN-kind.pub      the gate files
    pub\NN-kind.p2000.pub / .p98.pub   the same file saved in older Publisher formats (every 8th file)
    ref\NN-kind.pdf      Publisher's own PDF of each file (the fidelity reference)
    manifest.csv         one row per file: kind, pages, features, fonts, format, size, SHA-256
    corpus.log           what happened, file by file
  and finally foliolum-corpus.zip next to that folder, with its SHA-256 printed.

  It is safe to run again: files that already exist are kept unless -Force is given, and the zip is rebuilt.
  All content is invented. Names are fictional, phone numbers are 555-01xx, and addresses are example.org.

.PARAMETER Count
  How many gate files to make. Default 50. Use -Count 2 for a dry run.

.PARAMETER Force
  Rebuild files that already exist.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\make-corpus.ps1 -Count 2
  powershell -ExecutionPolicy Bypass -File .\make-corpus.ps1
#>
[CmdletBinding()]
param(
  [ValidateRange(1, 500)][int]$Count = 50,
  [switch]$Force
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$Root = Join-Path $env:USERPROFILE 'foliolum-corpus'
$PubDir = Join-Path $Root 'pub'
$RefDir = Join-Path $Root 'ref'
$PicDir = Join-Path $Root 'pictures'
$LogFile = Join-Path $Root 'corpus.log'
$Manifest = Join-Path $Root 'manifest.csv'
$ZipFile = Join-Path (Split-Path $Root -Parent) 'foliolum-corpus.zip'
foreach ($d in @($Root, $PubDir, $RefDir, $PicDir)) { New-Item -ItemType Directory -Force -Path $d | Out-Null }

function Log([string]$msg) {
  $line = '{0}  {1}' -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), $msg
  Write-Host $line
  Add-Content -Path $LogFile -Value $line -Encoding UTF8
}

# ---------------------------------------------------------------------------------------------------------
# Publisher enum values. Read from the Publisher interop assembly when it is installed; otherwise the
# fallback is used only where it is safe (marked), and the feature is skipped and logged where it is not.
# ---------------------------------------------------------------------------------------------------------
$Interop = $null
try { $Interop = [Reflection.Assembly]::LoadWithPartialName('Microsoft.Office.Interop.Publisher') } catch { }
function PbEnum([string]$type, [string]$name, $fallback) {
  if ($Interop) {
    $t = $Interop.GetType("Microsoft.Office.Interop.Publisher.$type", $false)
    if ($t) { return [int][Enum]::Parse($t, $name) }
  }
  return $fallback
}
$WrapSquare   = PbEnum 'PbWrapType' 'pbWrapTypeSquare' $null          # no safe fallback: skip if unknown
$WrapTight    = PbEnum 'PbWrapType' 'pbWrapTypeTight' $null
$FmtPub2000   = PbEnum 'PbFileFormat' 'pbFilePublisher2000' $null     # no safe fallback: skip if unknown
$FmtPub98     = PbEnum 'PbFileFormat' 'pbFilePublisher98' $null
$FixedPdf     = PbEnum 'PbFixedFormatType' 'pbFixedFormatTypePDF' 2   # checked: output must start with %PDF
$TextHoriz    = PbEnum 'PbTextOrientation' 'pbTextOrientationHorizontal' 1
$AlignCenter  = PbEnum 'PbParagraphAlignmentType' 'pbParagraphAlignmentCenter' $null
Log ("Publisher interop assembly: " + $(if ($Interop) { 'found' } else { 'not found (wrap, old formats and centring are skipped)' }))

# Office shape types (MsoAutoShapeType) are stable across Office versions.
$ShapeRect = 1; $ShapeRoundRect = 5; $ShapeOval = 9; $ShapeStar5 = 92

function RGB([int]$r, [int]$g, [int]$b) { return $r + 256 * $g + 65536 * $b }
function In([double]$inches) { return $inches * 72.0 }

# ---------------------------------------------------------------------------------------------------------
# Invented content
# ---------------------------------------------------------------------------------------------------------
$Places = @('St. Aurelian of the Hollow (sample)', 'Lantern Hill Community Church (sample)', 'Holy Name of the Orchard (sample)',
            'Riverbend Parish (sample)', 'Maple Street Fellowship (sample)', 'Our Lady of the Meadowlark (sample)')
$People = @('Ana Beltran', 'Joseph Okafor', 'Mary Kessler', 'Tom Huang', 'Rosa Delgado', 'Peter Lindqvist', 'Grace Mwangi',
            'Luis Ortega', 'Helen Brandt', 'Samuel Price', 'Teresa Novak', 'Daniel Abiola')
$Words = @('community', 'volunteers', 'parish', 'welcome', 'gathering', 'kitchen', 'choir', 'families', 'garden', 'youth',
           'retreat', 'Advent', 'Lent', 'bake sale', 'coffee', 'hall', 'library', 'mission', 'school', 'teachers', 'neighbors',
           'prayer', 'music', 'friends', 'season', 'Sunday', 'evening', 'weekend', 'collection', 'project', 'thank you')
$Spanish = @('Bienvenidos a nuestra comunidad parroquial.', 'La misa en español es el domingo a la 1:00 p.m.',
             'Gracias a todos los voluntarios por su ayuda.', 'Habrá café y pan dulce después de la misa.',
             'Las clases de catequesis comienzan en septiembre.')
$Extras = @('Tickets are $5 — children under 12 free.', 'The “Harvest Supper” is ½ price after 7 pm.',
            'Questions? Call 555-0142 or write to office@example.org.', 'Bring a friend… and a dish to share!',
            'Doors open at 6:30; seating is first-come, first-served.')
$FontSets = @(
  @{ Head = 'Cambria';         Body = 'Calibri' },
  @{ Head = 'Times New Roman'; Body = 'Times New Roman' },
  @{ Head = 'Arial Black';     Body = 'Arial' },
  @{ Head = 'Georgia';         Body = 'Garamond' },
  @{ Head = 'Century Gothic';  Body = 'Book Antiqua' },
  @{ Head = 'Lucida Handwriting'; Body = 'Segoe UI' }
)

function Sentence($rng) {
  $n = $rng.Next(8, 18); $w = @()
  for ($i = 0; $i -lt $n; $i++) { $w += $Words[$rng.Next($Words.Count)] }
  $s = ($w -join ' ')
  return $s.Substring(0, 1).ToUpper() + $s.Substring(1) + '.'
}
function Paragraph($rng, [int]$sentences) {
  $p = @(); for ($i = 0; $i -lt $sentences; $i++) { $p += Sentence $rng }
  if ($rng.Next(4) -eq 0) { $p += $Extras[$rng.Next($Extras.Count)] }
  return ($p -join ' ')
}
function Story($rng, [int]$paragraphs, [bool]$spanish) {
  $ps = @()
  for ($i = 0; $i -lt $paragraphs; $i++) {
    $ps += Paragraph $rng ($rng.Next(3, 7))
    if ($spanish -and $rng.Next(3) -eq 0) { $ps += $Spanish[$rng.Next($Spanish.Count)] }
  }
  return ($ps -join "`r")
}

# ---------------------------------------------------------------------------------------------------------
# Invented pictures, drawn with System.Drawing
# ---------------------------------------------------------------------------------------------------------
Add-Type -AssemblyName System.Drawing
function Make-Picture([string]$path, [int]$seed, [string]$label) {
  if ((Test-Path $path) -and -not $Force) { return }
  $rng = New-Object System.Random $seed
  $bmp = New-Object System.Drawing.Bitmap 800, 600
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'
  $c1 = [System.Drawing.Color]::FromArgb(255, $rng.Next(40, 220), $rng.Next(40, 220), $rng.Next(40, 220))
  $c2 = [System.Drawing.Color]::FromArgb(255, $rng.Next(40, 220), $rng.Next(40, 220), $rng.Next(40, 220))
  $rect = New-Object System.Drawing.Rectangle 0, 0, 800, 600
  $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $c1, $c2, 45.0
  $g.FillRectangle($brush, $rect)
  for ($i = 0; $i -lt 6; $i++) {
    $b = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(110, $rng.Next(256), $rng.Next(256), $rng.Next(256)))
    $g.FillEllipse($b, $rng.Next(-100, 700), $rng.Next(-100, 500), $rng.Next(120, 360), $rng.Next(120, 360))
    $b.Dispose()
  }
  $font = New-Object System.Drawing.Font 'Segoe UI', 36, ([System.Drawing.FontStyle]::Bold)
  $g.DrawString($label, $font, [System.Drawing.Brushes]::White, 40, 500)
  $fmt = if ($path.EndsWith('.jpg')) { [System.Drawing.Imaging.ImageFormat]::Jpeg } else { [System.Drawing.Imaging.ImageFormat]::Png }
  $bmp.Save($path, $fmt)
  $font.Dispose(); $brush.Dispose(); $g.Dispose(); $bmp.Dispose()
}
$Pictures = @()
for ($i = 1; $i -le 12; $i++) {
  $ext = if ($i % 3 -eq 0) { 'jpg' } else { 'png' }
  $p = Join-Path $PicDir ("picture-{0:D2}.{1}" -f $i, $ext)
  Make-Picture $p (1000 + $i) ("Sample picture $i")
  $Pictures += $p
}

# ---------------------------------------------------------------------------------------------------------
# Building blocks. Each one records the features it used so the manifest can say what a file exercises.
# ---------------------------------------------------------------------------------------------------------
function Add-Text($page, [double]$x, [double]$y, [double]$w, [double]$h, [string]$text, [string]$font, [double]$size, [bool]$bold) {
  $s = $page.Shapes.AddTextbox($TextHoriz, (In $x), (In $y), (In $w), (In $h))
  $r = $s.TextFrame.TextRange
  $r.Text = $text
  $r.Font.Name = $font; $r.Font.Size = $size; $r.Font.Bold = [int]$bold
  return $s
}
function Try-Feature([System.Collections.Generic.HashSet[string]]$used, [string]$name, [scriptblock]$action) {
  try { & $action; [void]$used.Add($name) } catch { Log ("    skipped $name : " + $_.Exception.Message) }
}
function Add-Bullets($page, [double]$x, [double]$y, [double]$w, [double]$h, [string[]]$items, [string]$font, $used) {
  # Wingdings "l" is a round bullet: this exercises symbol-font mapping.
  $s = Add-Text $page $x $y $w $h (($items | ForEach-Object { "l`t$_" }) -join "`r") $font 10 $false
  Try-Feature $used 'symbol-font-bullets' {
    $r = $s.TextFrame.TextRange; $pos = 1
    foreach ($it in $items) { $r.Characters($pos, 1).Font.Name = 'Wingdings'; $pos += $it.Length + 3 }
  }
  return $s
}
function Add-Picture($page, [string]$file, [double]$x, [double]$y, [double]$w, [double]$h, $used, [bool]$wrap) {
  $pic = $page.Shapes.AddPicture($file, 0, -1, (In $x), (In $y), (In $w), (In $h))
  [void]$used.Add('picture')
  if ($wrap -and $WrapSquare -ne $null) { Try-Feature $used 'text-wrap' { $pic.TextWrap.Type = $WrapSquare } }
  return $pic
}
function Add-Table($page, [double]$x, [double]$y, [double]$w, [double]$h, [string[][]]$rows, [string]$font, $used) {
  $t = $page.Shapes.AddTable($rows.Count, $rows[0].Count, (In $x), (In $y), (In $w), (In $h))
  for ($r = 0; $r -lt $rows.Count; $r++) {
    for ($c = 0; $c -lt $rows[$r].Count; $c++) {
      $cell = $t.Table.Cells.Item($r + 1, $c + 1)
      $cell.TextRange.Text = $rows[$r][$c]
      $cell.TextRange.Font.Name = $font; $cell.TextRange.Font.Size = 9
      if ($r -eq 0) { $cell.TextRange.Font.Bold = 1 }
    }
  }
  [void]$used.Add('table')
  Try-Feature $used 'table-cell-fill' { for ($c = 1; $c -le $rows[0].Count; $c++) { $t.Table.Cells.Item(1, $c).Fill.ForeColor.RGB = (RGB 222 230 240) } }
  return $t
}
function Add-Master($doc, [string]$footer, [string]$font, $used) {
  $m = $doc.MasterPages.Item(1)
  $w = $doc.PageSetup.PageWidth / 72.0; $h = $doc.PageSetup.PageHeight / 72.0
  $f = Add-Text $m 0.5 ($h - 0.55) ($w - 1.0) 0.3 $footer $font 8 $false
  [void]$used.Add('master-page')
  Try-Feature $used 'page-number-field' {
    $n = $m.Shapes.AddTextbox($TextHoriz, (In ($w - 1.3)), (In ($h - 0.55)), (In 0.8), (In 0.3))
    $n.TextFrame.TextRange.InsertPageNumber() | Out-Null
    $n.TextFrame.TextRange.InsertBefore('Page ') | Out-Null
    $n.TextFrame.TextRange.Font.Name = $font; $n.TextFrame.TextRange.Font.Size = 8
  }
  $line = $m.Shapes.AddLine((In 0.5), (In ($h - 0.62)), (In ($w - 0.5)), (In ($h - 0.62)))
  $line.Line.Weight = 0.75
  [void]$used.Add('line')
}
function Add-Linked-Story($doc, [int[]]$pageNumbers, [string]$story, [string]$font, [int]$columns, $used) {
  # One story through a chain of boxes, one per page: linked text frames and overflow.
  $w = $doc.PageSetup.PageWidth / 72.0; $h = $doc.PageSetup.PageHeight / 72.0
  $prev = $null; $first = $null
  foreach ($pn in $pageNumbers) {
    $p = $doc.Pages.Item($pn)
    $box = $p.Shapes.AddTextbox($TextHoriz, (In 0.6), (In ($h * 0.45)), (In ($w - 1.2)), (In ($h * 0.42)))
    if ($columns -gt 1) { Try-Feature $used 'columns' { $box.TextFrame.Columns = $columns } }
    if ($prev -ne $null) { $prev.TextFrame.NextLinkedTextFrame = $box.TextFrame; [void]$used.Add('linked-text-boxes') }
    else { $first = $box }
    $prev = $box
  }
  $first.TextFrame.TextRange.Text = $story
  $first.TextFrame.TextRange.Font.Name = $font
  $first.TextFrame.TextRange.Font.Size = 10
}
function Add-WordArt($page, [string]$text, [string]$font, [double]$x, [double]$y, [int]$effect, $used) {
  Try-Feature $used 'wordart' { $page.Shapes.AddTextEffect($effect, $text, $font, 36, -1, 0, (In $x), (In $y)) | Out-Null }
}
function Add-Shapes($page, $rng, [double]$w, [double]$h, $used) {
  $band = $page.Shapes.AddShape($ShapeRect, 0, 0, (In $w), (In 0.35))
  $band.Fill.ForeColor.RGB = (RGB $rng.Next(60, 200) $rng.Next(60, 200) $rng.Next(60, 200)); $band.Line.Visible = 0
  $badge = $page.Shapes.AddShape(@($ShapeOval, $ShapeStar5, $ShapeRoundRect)[$rng.Next(3)], (In ($w - 1.6)), (In 0.5), (In 1.1), (In 1.1))
  $badge.Fill.ForeColor.RGB = (RGB 179 50 42); $badge.Line.Weight = 1.5
  Try-Feature $used 'shape-text' { $badge.TextFrame.TextRange.Text = 'New!'; $badge.TextFrame.TextRange.Font.Bold = 1 }
  [void]$used.Add('shapes')
  Try-Feature $used 'group' { $page.Shapes.Range([object[]]@($band.Name, $badge.Name)).Group() | Out-Null }
}

# ---------------------------------------------------------------------------------------------------------
# Kinds of publication
# ---------------------------------------------------------------------------------------------------------
function Build-Publication($pub, [int]$index, [string]$kind, $rng, $fonts, $used) {
  $doc = $pub.NewDocument()
  $place = $Places[$index % $Places.Count]
  $spanish = ($index % 3 -eq 0)
  switch ($kind) {
    'bulletin' { $pw = 5.5; $ph = 8.5; $pages = @(8, 12)[$index % 2] }
    'newsletter' { $pw = 8.5; $ph = 11; $pages = 4 }
    'flyer' { $pw = 8.5; $ph = 11; $pages = 1 }
    'program' { $pw = 5.5; $ph = 8.5; $pages = 4 }
    'brochure' { $pw = 11; $ph = 8.5; $pages = 2 }
  }
  $doc.PageSetup.PageWidth = In $pw; $doc.PageSetup.PageHeight = In $ph
  while ($doc.Pages.Count -lt $pages) { $doc.Pages.Add(1, $doc.Pages.Count) | Out-Null }
  if ($pages -gt 1) { Try-Feature $used 'two-page-spread' { $doc.ViewTwoPageSpread = -1 } }
  if ($pages -gt 1) { Add-Master $doc ("$place · 1 Sample Road · 555-01{0:D2} · office@example.org" -f ($index % 100)) $fonts.Body $used }

  $cover = $doc.Pages.Item(1)
  Add-Shapes $cover $rng $pw $ph $used
  $title = switch ($kind) { 'bulletin' { 'Weekly Bulletin' } 'newsletter' { 'Community News' } 'flyer' { 'Harvest Supper' } 'program' { 'Order of Celebration' } 'brochure' { 'Welcome' } }
  if ($index % 2 -eq 0) { Add-WordArt $cover $title $fonts.Head 0.5 0.6 ($index % 10) $used }
  else { $t = Add-Text $cover 0.5 0.6 ($pw - 2.2) 0.9 $title $fonts.Head 28 $true; if ($AlignCenter -ne $null) { Try-Feature $used 'centered' { $t.TextFrame.TextRange.ParagraphFormat.Alignment = $AlignCenter } } }
  Add-Text $cover 0.5 1.5 ($pw - 1.0) 0.4 ("{0} — {1:MMMM d, yyyy}" -f $place, (Get-Date '2026-12-06').AddDays(7 * $index)) $fonts.Body 11 $false | Out-Null
  Add-Picture $cover $Pictures[$index % $Pictures.Count] ($pw - 2.6) 2.1 2.1 1.6 $used $true | Out-Null
  Add-Text $cover 0.5 2.1 ($pw - 3.3) 2.0 (Paragraph $rng 4) $fonts.Body 10 $false | Out-Null

  if ($kind -in @('bulletin', 'program')) {
    $rows = @(, [string[]]@('Day', 'Time', 'Notes'))
    $rows += , [string[]]@('Saturday', '5:00 pm', 'Vigil')
    $rows += , [string[]]@('Sunday', '8:00 am', '')
    $rows += , [string[]]@('Sunday', '10:30 am', 'Choir')
    if ($spanish) { $rows += , [string[]]@('Domingo', '1:00 p.m.', 'En español') }
    Add-Table $cover 0.5 4.4 ($pw - 1.0) 1.3 $rows $fonts.Body $used | Out-Null
  }
  if ($pages -gt 1) {
    $chain = @(2..[Math]::Min($pages, 4))
    Add-Linked-Story $doc $chain (Story $rng (10 + $pages) $spanish) $fonts.Body @(1, 2, 3)[$index % 3] $used
    for ($pn = 2; $pn -le $pages; $pn++) {
      $p = $doc.Pages.Item($pn)
      Add-Text $p 0.6 0.5 ($pw - 1.2) 0.5 ("{0}" -f (Sentence $rng)).Substring(0, 30) $fonts.Head 16 $true | Out-Null
      if ($pn % 2 -eq 0) { Add-Picture $p $Pictures[($index + $pn) % $Pictures.Count] 0.6 1.1 ($pw * 0.45) ($ph * 0.25) $used $true | Out-Null }
      else { Add-Bullets $p 0.6 1.1 ($pw - 1.2) 1.6 @($People[$rng.Next($People.Count)] + ' — lector', (Sentence $rng), $Extras[$rng.Next($Extras.Count)]) $fonts.Body $used | Out-Null }
    }
    if ($kind -eq 'bulletin') {
      # Ad grid on the last page: the self-printing parish case.
      $last = $doc.Pages.Item($pages)
      for ($a = 0; $a -lt 4; $a++) {
        $ax = 0.5 + ($a % 2) * (($pw - 1.0) / 2); $ay = $ph - 3.2 + [Math]::Floor($a / 2) * 1.2
        $ad = $last.Shapes.AddShape($ShapeRect, (In $ax), (In $ay), (In (($pw - 1.2) / 2)), (In 1.0))
        $ad.Fill.ForeColor.RGB = (RGB 250 248 242); $ad.Line.Weight = 0.5
        $ad.TextFrame.TextRange.Text = ("Sample Business {0}`r555-01{1:D2}" -f ($a + 1), (($index + $a) % 100))
        $ad.TextFrame.TextRange.Font.Size = 8; $ad.TextFrame.TextRange.Font.Name = $fonts.Body
      }
      [void]$used.Add('ad-grid')
    }
  }
  return $doc
}

# ---------------------------------------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------------------------------------
$Kinds = @('bulletin', 'newsletter', 'flyer', 'program', 'brochure')
Log "make-corpus: Count=$Count Force=$Force Root=$Root"
$pub = New-Object -ComObject Publisher.Application
$rows = New-Object System.Collections.Generic.List[object]
$failed = 0
try {
  for ($i = 1; $i -le $Count; $i++) {
    $kind = $Kinds[($i - 1) % $Kinds.Count]
    $name = '{0:D2}-{1}' -f $i, $kind
    $file = Join-Path $PubDir "$name.pub"
    $ref = Join-Path $RefDir "$name.pdf"
    $fonts = $FontSets[($i - 1) % $FontSets.Count]
    $used = New-Object 'System.Collections.Generic.HashSet[string]'
    $variants = @()
    if ((Test-Path $file) -and -not $Force) {
      Log "$name : exists, kept"
    } else {
      Log "$name : building ($($fonts.Head) / $($fonts.Body))"
      $doc = $null
      try {
        $rng = New-Object System.Random (5000 + $i)
        $doc = Build-Publication $pub $i $kind $rng $fonts $used
        if (Test-Path $file) { Remove-Item $file }
        $doc.SaveAs($file)
        Try-Feature $used 'reference-pdf' {
          if (Test-Path $ref) { Remove-Item $ref }
          $doc.ExportAsFixedFormat($FixedPdf, $ref)
          $head = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($ref)[0..3])
          if ($head -ne '%PDF') { Remove-Item $ref; throw 'export did not produce a PDF' }
        }
        if ($i % 8 -eq 0) {
          foreach ($v in @(@{ Fmt = $FmtPub2000; Ext = 'p2000' }, @{ Fmt = $FmtPub98; Ext = 'p98' })) {
            if ($v.Fmt -eq $null) { Log "    skipped .$($v.Ext).pub : format value unknown without the interop assembly"; continue }
            $vf = Join-Path $PubDir "$name.$($v.Ext).pub"
            try { if (Test-Path $vf) { Remove-Item $vf }; $doc.SaveAs($vf, $v.Fmt); $variants += $vf; Log "    saved $(Split-Path $vf -Leaf)" }
            catch { Log "    skipped .$($v.Ext).pub : $($_.Exception.Message)" }
          }
        }
        Log ("    features: " + (($used | Sort-Object) -join ', '))
      } catch {
        $failed++
        Log "$name : FAILED : $($_.Exception.Message)"
      } finally {
        if ($doc -ne $null) { try { $doc.Close() } catch { } ; [void][Runtime.InteropServices.Marshal]::ReleaseComObject($doc) }
      }
    }
    foreach ($f in @($file) + $variants) {
      if (Test-Path $f) {
        $rows.Add([pscustomobject]@{
          file = Split-Path $f -Leaf; kind = $kind
          format = $(if ($f -like '*.p2000.pub') { 'publisher-2000' } elseif ($f -like '*.p98.pub') { 'publisher-98' } else { 'current' })
          fonts = "$($fonts.Head);$($fonts.Body)"; features = (($used | Sort-Object) -join ';')
          reference_pdf = $(if (Test-Path $ref) { Split-Path $ref -Leaf } else { '' })
          bytes = (Get-Item $f).Length; sha256 = (Get-FileHash $f -Algorithm SHA256).Hash.ToLower()
        })
      }
    }
  }
} finally {
  try { $pub.Quit() } catch { }
  [void][Runtime.InteropServices.Marshal]::ReleaseComObject($pub)
}

# A kept file has no feature list from this run; keep the previous manifest's entry for it.
if (Test-Path $Manifest) {
  $old = @{}; Import-Csv $Manifest | ForEach-Object { $old[$_.file] = $_ }
  foreach ($r in $rows) { if (-not $r.features -and $old.ContainsKey($r.file)) { $r.features = $old[$r.file].features } }
}
$rows | Export-Csv -Path $Manifest -NoTypeInformation -Encoding UTF8
if (Test-Path $ZipFile) { Remove-Item $ZipFile }
Compress-Archive -Path (Join-Path $Root '*') -DestinationPath $ZipFile
$hash = (Get-FileHash $ZipFile -Algorithm SHA256).Hash.ToLower()
Log "done: $($rows.Count) file(s) in manifest, $failed failed"
Log "zip: $ZipFile"
Log "sha256: $hash"
Write-Host ''
Write-Host "Send foliolum-corpus.zip and this hash to the Foliolum session:" -ForegroundColor Cyan
Write-Host "  $hash" -ForegroundColor Cyan
if ($failed -gt 0) { exit 2 }
