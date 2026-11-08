<?php

declare(strict_types=1);

// Compara Nbt::anonymous() con los binarios de referencia de prismarine-nbt.
require __DIR__ . '/../src/javabridge/Nbt.php';

use javabridge\Nbt;

$manifest = __DIR__ . '/../../../target/nbtcheck/expected.json';
if(!is_file($manifest)){
	fwrite(STDERR, "Falta {$manifest}; ejecuta node target/nbtcheck/validate.js\n");
	exit(1);
}

$cases = json_decode((string)file_get_contents($manifest), true, 512, JSON_THROW_ON_ERROR);
$failures = 0;

foreach($cases as $case){
	$actual = Nbt::anonymous($case['node']);
	$expected = (string)$case['hex'];
	$actualHex = bin2hex($actual);
	if($actualHex !== $expected){
		$failures++;
		fwrite(STDERR, "FAIL {$case['name']}\n  esperado {$expected}\n  obtenido {$actualHex}\n");
	}else{
		fwrite(STDOUT, "ok: {$case['name']}\n");
	}
}

fwrite(STDOUT, $failures === 0 ? "ALL OK (" . count($cases) . " casos)\n" : "{$failures} FAILURE(S)\n");
exit($failures === 0 ? 0 : 1);
