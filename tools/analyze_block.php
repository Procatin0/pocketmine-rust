<?php
declare(strict_types=1);

$baseDir = __DIR__ . '/../target/axolotl-pm/src/block';

// 1. Parse VanillaBlocksInputs.php to find registrations
$vbiContent = file_get_contents($baseDir . '/VanillaBlocksInputs.php');
$vbiTokens = PhpToken::tokenize($vbiContent);

$vbiBlocks = []; // name => ['class' => ..., 'tile' => ...]
$vbiInstantiatedClasses = []; // class_short_name => count

// Regex / token scan for: self::register("name", fn(BID $id) => new ClassName(...), ?TileClass::class)
// or register(...)
preg_match_all('/self::register\(\s*"([^"]+)",\s*(?:fn\([^)]*\)\s*=>\s*new\s+([A-Za-z0-9_]+)|function\([^)]*\)[^}]*new\s+([A-Za-z0-9_]+))(?:[^;]*?Tile([A-Za-z0-9_]+)::class)?/s', $vbiContent, $matches, PREG_SET_ORDER);

// Also more precise token scanner for VanillaBlocksInputs
$inRegister = false;
$currentBlockName = null;
$registeredBlocks = [];

for ($i = 0; $i < count($vbiTokens); $i++) {
    $t = $vbiTokens[$i];
    if ($t->is(T_STRING) && $t->text === 'register' && isset($vbiTokens[$i-1]) && $vbiTokens[$i-1]->is(T_PAAMAYIM_NEKUDOTAYIM)) {
        // Look ahead for string argument
        $j = $i + 1;
        while ($j < count($vbiTokens) && !$vbiTokens[$j]->is(T_CONSTANT_ENCAPSED_STRING)) {
            $j++;
        }
        if ($j < count($vbiTokens)) {
            $bName = trim($vbiTokens[$j]->text, "\"'");
            // Find `new ClassName`
            $className = null;
            $tileClass = null;
            while ($j < count($vbiTokens) && $vbiTokens[$j]->text !== ';') {
                if ($vbiTokens[$j]->is(T_NEW) && isset($vbiTokens[$j+2]) && $vbiTokens[$j+2]->is(T_STRING)) {
                    $className = $vbiTokens[$j+2]->text;
                }
                if ($vbiTokens[$j]->is(T_CLASS) && isset($vbiTokens[$j-2]) && $vbiTokens[$j-2]->is(T_STRING)) {
                    $prev = $vbiTokens[$j-2]->text;
                    if (str_starts_with($prev, 'Tile')) {
                        $tileClass = $prev;
                    }
                }
                $j++;
            }
            if ($className !== null) {
                $registeredBlocks[$bName] = [
                    'class' => $className,
                    'tile' => $tileClass,
                ];
                $vbiInstantiatedClasses[$className] = ($vbiInstantiatedClasses[$className] ?? 0) + 1;
            }
        }
    }
}

echo "VanillaBlocksInputs registered blocks count: " . count($registeredBlocks) . "\n";
echo "VanillaBlocksInputs instantiated distinct classes count: " . count($vbiInstantiatedClasses) . "\n";

// 2. Scan all PHP files in target/axolotl-pm/src/block
$iterator = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($baseDir));
$phpFiles = [];
foreach ($iterator as $file) {
    if ($file->isFile() && $file->getExtension() === 'php') {
        $phpFiles[] = $file->getPathname();
    }
}
sort($phpFiles);

$declarations = [];

foreach ($phpFiles as $filePath) {
    $relPath = str_replace('\\', '/', substr($filePath, strlen(dirname($baseDir)) + 1));
    $code = file_get_contents($filePath);
    $tokens = PhpToken::tokenize($code);
    
    $namespace = '';
    $uses = [];
    $count = count($tokens);
    
    for ($i = 0; $i < $count; $i++) {
        $t = $tokens[$i];
        if ($t->is(T_NAMESPACE)) {
            $ns = '';
            $j = $i + 1;
            while ($j < $count && $tokens[$j]->text !== ';') {
                $ns .= $tokens[$j]->text;
                $j++;
            }
            $namespace = trim($ns);
            $i = $j;
            continue;
        }
        
        if ($t->is(T_USE) && !isset($inClass)) {
            // Check if it's class use (not inside class body which would be trait use)
            // But we track trait use vs file use by whether we are in a class body
            // Handled below
        }
        
        // Check for class, interface, trait, enum
        $isClass = $t->is(T_CLASS);
        $isInterface = $t->is(T_INTERFACE);
        $isTrait = $t->is(T_TRAIT);
        $isEnum = $t->is(T_ENUM);
        
        if ($isClass || $isInterface || $isTrait || $isEnum) {
            // Ensure not `class` keyword used in ::class or anonymous class `new class`
            if ($i > 0 && $tokens[$i-1]->is(T_PAAMAYIM_NEKUDOTAYIM)) {
                continue;
            }
            if ($i > 1 && ($tokens[$i-1]->is(T_NEW) || ($tokens[$i-2]->is(T_NEW) && $tokens[$i-1]->is(T_WHITESPACE)))) {
                // Anonymous class
                continue;
            }
            
            // Find declaration name
            $j = $i + 1;
            while ($j < $count && !$tokens[$j]->is(T_STRING)) {
                $j++;
            }
            if ($j >= $count) continue;
            $declName = $tokens[$j]->text;
            $fqcn = ($namespace ? $namespace . '\\' : '') . $declName;
            
            // Find extends & implements before `{`
            $extends = '';
            $implements = [];
            $k = $j + 1;
            $mode = null;
            while ($k < $count && $tokens[$k]->text !== '{') {
                if ($tokens[$k]->is(T_EXTENDS)) {
                    $mode = 'extends';
                    $k++;
                    continue;
                }
                if ($tokens[$k]->is(T_IMPLEMENTS)) {
                    $mode = 'implements';
                    $k++;
                    continue;
                }
                if ($mode === 'extends') {
                    if ($tokens[$k]->is([T_STRING, T_NAME_QUALIFIED, T_NAME_FULLY_QUALIFIED])) {
                        $extends .= $tokens[$k]->text;
                    }
                } elseif ($mode === 'implements') {
                    if ($tokens[$k]->is([T_STRING, T_NAME_QUALIFIED, T_NAME_FULLY_QUALIFIED])) {
                        $implements[] = $tokens[$k]->text;
                    }
                }
                $k++;
            }
            
            if ($k >= $count) continue;
            
            // Now $k is '{', the start of class body!
            $bodyStart = $k;
            $braceDepth = 1;
            $bodyTokens = [];
            $k++;
            while ($k < $count && $braceDepth > 0) {
                if ($tokens[$k]->text === '{') {
                    $braceDepth++;
                } elseif ($tokens[$k]->text === '}') {
                    $braceDepth--;
                    if ($braceDepth === 0) break;
                }
                $bodyTokens[] = $tokens[$k];
                $k++;
            }
            $bodyEnd = $k;
            $i = $k; // advance outer loop past this class body
            
            // Analyze methods inside bodyTokens
            $publicMethods = [];
            $bCount = count($bodyTokens);
            $innerDepth = 0;
            
            for ($b = 0; $b < $bCount; $b++) {
                $bt = $bodyTokens[$b];
                if ($bt->text === '{') {
                    $innerDepth++;
                } elseif ($bt->text === '}') {
                    $innerDepth--;
                }
                
                // Only consider methods at root level of class body (depth 0)
                if ($innerDepth === 0 && $bt->is(T_FUNCTION)) {
                    // Look backward to check visibility modifiers
                    $isPublic = true; // default in interface, check class/trait
                    $isPrivate = false;
                    $isProtected = false;
                    
                    // Look back up to previous semicolon or closing brace or open brace
                    for ($m = $b - 1; $m >= 0; $m--) {
                        $prev = $bodyTokens[$m];
                        if ($prev->text === ';' || $prev->text === '}' || $prev->text === '{') {
                            break;
                        }
                        if ($prev->is(T_PRIVATE)) {
                            $isPrivate = true;
                            $isPublic = false;
                        }
                        if ($prev->is(T_PROTECTED)) {
                            $isProtected = true;
                            $isPublic = false;
                        }
                        if ($prev->is(T_PUBLIC)) {
                            $isPublic = true;
                        }
                    }
                    
                    if ($isPublic && !$isPrivate && !$isProtected) {
                        // Find function name
                        $fn = $b + 1;
                        while ($fn < $bCount && !$bodyTokens[$fn]->is(T_STRING)) {
                            $fn++;
                        }
                        if ($fn < $bCount) {
                            $publicMethods[] = $bodyTokens[$fn]->text;
                        }
                    }
                }
            }
            
            // Check dependencies in class body & file
            $bodyText = '';
            foreach ($bodyTokens as $bt) {
                $bodyText .= $bt->text;
            }
            
            $hasItem = (bool)preg_match('/\bItem\b/', $bodyText) || (bool)preg_match('/use\s+[^;]*\bItem\b/', $code);
            $hasWorld = (bool)preg_match('/\bWorld\b/', $bodyText) || (bool)preg_match('/use\s+[^;]*\bWorld\b/', $code);
            $hasPlayer = (bool)preg_match('/\bPlayer\b/', $bodyText) || (bool)preg_match('/use\s+[^;]*\bPlayer\b/', $code);
            $hasTile = (bool)preg_match('/\bTile\b/', $bodyText) || (bool)preg_match('/use\s+[^;]*\btile\\\\/', $code);
            $hasInventory = (bool)preg_match('/\bInventory\b/', $bodyText) || (bool)preg_match('/use\s+[^;]*\bInventory\b/', $code);
            
            $kind = $isClass ? 'class' : ($isInterface ? 'interface' : ($isTrait ? 'trait' : 'enum'));
            
            $isInstantiatedInVBI = isset($vbiInstantiatedClasses[$declName]);
            
            $declarations[] = [
                'fqcn' => $fqcn,
                'name' => $declName,
                'file' => $relPath,
                'kind' => $kind,
                'extends' => $extends ?: '-',
                'implements' => !empty($implements) ? implode(', ', $implements) : '-',
                'public_methods_count' => count($publicMethods),
                'public_methods' => $publicMethods,
                'vbi_instantiated' => $isInstantiatedInVBI,
                'has_tile' => $hasTile,
                'dep_item' => $hasItem,
                'dep_world' => $hasWorld,
                'dep_player' => $hasPlayer,
                'dep_tile' => $hasTile,
                'dep_inventory' => $hasInventory,
            ];
        }
    }
}

file_put_contents(__DIR__ . '/block_inventory.json', json_encode([
    'total_declarations' => count($declarations),
    'registered_blocks_count' => count($registeredBlocks),
    'registered_blocks' => $registeredBlocks,
    'declarations' => $declarations,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES));

echo "Total declarations found: " . count($declarations) . "\n";
