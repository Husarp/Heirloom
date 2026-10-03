# Downloads the official GEDCOM 7 test files from gedcom.io (FamilySearch) into test-files\gedcom.io, where
# crates\heirloom-core\tests\gedcom_io.rs and crates\heirloom-api\tests\gedcom_io.rs pick them up. They are kept out
# of Git; without them those two tests pass without checking anything. Run from any folder:
#   & "<project folder>\scripts\fetch-gedcom-test-files.ps1"
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Target = "$Root\test-files\gedcom.io"
New-Item -ItemType Directory -Force $Target | Out-Null
$files = "age", "escapes", "extension-record", "extensions", "filename-1", "lang", "long-url", "maximal70",
    "maximal70-lds", "maximal70-memories1", "maximal70-memories2", "maximal70-tree1", "maximal70-tree2", "minimal70",
    "notes-1", "obje-1", "remarriage1", "remarriage2", "same-sex-marriage", "voidptr", "warnings70", "xref" |
    ForEach-Object { "$_.ged" }
$files += "maximal70.gdz", "minimal70.gdz"
foreach ($file in $files) {
    Invoke-WebRequest "https://gedcom.io/testfiles/gedcom70/$file" -OutFile "$Target\$file" -UseBasicParsing
}
Write-Output "Downloaded $($files.Count) files into $Target"
