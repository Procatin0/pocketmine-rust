<?php
$file = 'target/axolotl-pm/src/block/Block.php';
$tokens = PhpToken::tokenize(file_get_contents($file));
echo "Tokens in Block.php: " . count($tokens) . "\n";
