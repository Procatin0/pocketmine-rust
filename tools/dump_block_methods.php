<?php
declare(strict_types=1);

$code = file_get_contents(__DIR__ . '/../target/axolotl-pm/src/block/Block.php');
$tokens = PhpToken::tokenize($code);

$methods = [];
$count = count($tokens);
$depth = 0;

for ($i = 0; $i < $count; $i++) {
    $t = $tokens[$i];
    if ($t->text === '{') $depth++;
    elseif ($t->text === '}') $depth--;
    
    // Class-level methods (depth 1)
    if ($depth === 1 && $t->is(T_FUNCTION)) {
        // Collect modifiers backward
        $modifiers = [];
        for ($j = $i - 1; $j >= 0; $j--) {
            if ($tokens[$j]->text === ';' || $tokens[$j]->text === '}' || $tokens[$j]->text === '{') break;
            if ($tokens[$j]->is([T_PUBLIC, T_PROTECTED, T_PRIVATE, T_STATIC, T_FINAL, T_ABSTRACT])) {
                $modifiers[] = $tokens[$j]->text;
            }
        }
        $modifiers = array_reverse($modifiers);
        $visibility = in_array('private', $modifiers) ? 'private' : (in_array('protected', $modifiers) ? 'protected' : 'public');
        
        // Find method name
        $k = $i + 1;
        while ($k < $count && !$tokens[$k]->is(T_STRING)) $k++;
        $methodName = $tokens[$k]->text;
        
        // Find parameters and return type until { or ;
        $sigTokens = [];
        $pDepth = 0;
        while ($k < $count && ($tokens[$k]->text !== '{' && $tokens[$k]->text !== ';')) {
            $sigTokens[] = $tokens[$k]->text;
            $k++;
        }
        $sig = implode('', $sigTokens);
        
        $methods[] = [
            'name' => $methodName,
            'visibility' => $visibility,
            'modifiers' => implode(' ', $modifiers),
            'signature' => $sig,
        ];
    }
}

file_put_contents(__DIR__ . '/block_methods.json', json_encode($methods, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES));
echo "Found " . count($methods) . " methods in Block.php.\n";
$publicMethods = array_filter($methods, fn($m) => $m['visibility'] === 'public');
echo "Public methods: " . count($publicMethods) . "\n";
foreach ($publicMethods as $pm) {
    echo $pm['modifiers'] . ' function ' . $pm['signature'] . "\n";
}
