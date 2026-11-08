<?php

declare(strict_types=1);

namespace javabridge;

/**
 * Escritor de NBT en formato de red (NBT "anónimo": tipo + payload, sin nombre
 * de raíz), que es el que usan los registros y los componentes de texto de las
 * versiones modernas de Java.
 *
 * Trabaja sobre el mismo JSON tipado que usa protodef:
 *   {"type":"compound","value":{"clave":{"type":"int","value":1}}}
 *   {"type":"list","value":{"type":"string","value":["a","b"]}}
 * Los long se aceptan como int de PHP o como par [alto, bajo] (así los
 * serializa protodef en JavaScript).
 */
final class Nbt{

	public const TAG_END = 0;
	public const TAG_BYTE = 1;
	public const TAG_SHORT = 2;
	public const TAG_INT = 3;
	public const TAG_LONG = 4;
	public const TAG_FLOAT = 5;
	public const TAG_DOUBLE = 6;
	public const TAG_BYTE_ARRAY = 7;
	public const TAG_STRING = 8;
	public const TAG_LIST = 9;
	public const TAG_COMPOUND = 10;
	public const TAG_INT_ARRAY = 11;
	public const TAG_LONG_ARRAY = 12;

	private const TAGS = [
		'end' => self::TAG_END,
		'byte' => self::TAG_BYTE,
		'short' => self::TAG_SHORT,
		'int' => self::TAG_INT,
		'long' => self::TAG_LONG,
		'float' => self::TAG_FLOAT,
		'double' => self::TAG_DOUBLE,
		'byteArray' => self::TAG_BYTE_ARRAY,
		'string' => self::TAG_STRING,
		'list' => self::TAG_LIST,
		'compound' => self::TAG_COMPOUND,
		'intArray' => self::TAG_INT_ARRAY,
		'longArray' => self::TAG_LONG_ARRAY,
	];

	/**
	 * Serializa un nodo como NBT de red: byte de tipo + payload, sin nombre.
	 *
	 * @param array{type:string, value:mixed} $node
	 */
	public static function anonymous(array $node) : string{
		return chr(self::tagOf((string)$node['type'])) . self::payload($node);
	}

	/** Componente de texto plano: {"text": "..."}. */
	public static function text(string $text) : string{
		return self::anonymous([
			'type' => 'compound',
			'value' => ['text' => ['type' => 'string', 'value' => $text]],
		]);
	}

	/** Compound con un único long array (usado por los heightmaps). */
	public static function longArrayCompound(string $key, array $values) : string{
		return self::anonymous([
			'type' => 'compound',
			'value' => [$key => ['type' => 'longArray', 'value' => $values]],
		]);
	}

	private static function tagOf(string $type) : int{
		$tag = self::TAGS[$type] ?? null;
		if($tag === null){
			throw new \InvalidArgumentException("Tipo NBT desconocido: {$type}");
		}
		return $tag;
	}

	/** @param array{type:string, value:mixed} $node */
	private static function payload(array $node) : string{
		$type = (string)$node['type'];
		$value = $node['value'] ?? null;
		if($type === 'end'){
			return '';
		}

		if($type === 'compound'){
			$out = '';
			foreach((array)$value as $name => $child){
				$out .= chr(self::tagOf((string)$child['type'])) . self::string((string)$name) . self::payload($child);
			}
			return $out . chr(self::TAG_END);
		}

		if($type === 'list'){
			$inner = (array)$value;
			$elementType = (string)($inner['type'] ?? 'end');
			$elements = (array)($inner['value'] ?? []);
			$out = chr(self::tagOf($elementType)) . pack('N', count($elements));
			foreach($elements as $element){
				$out .= $elementType === 'compound' ? self::payload(['type' => 'compound', 'value' => $element]) : self::payload(['type' => $elementType, 'value' => $element]);
			}
			return $out;
		}

		if($type === 'byte'){
			return chr(((int)$value) & 0xFF);
		}
		if($type === 'short'){
			return pack('n', ((int)$value) & 0xFFFF);
		}
		if($type === 'int'){
			return pack('N', ((int)$value) & 0xFFFFFFFF);
		}
		if($type === 'long'){
			return pack('J', self::longValue($value));
		}
		if($type === 'float'){
			return pack('G', (float)$value);
		}
		if($type === 'double'){
			return pack('E', (float)$value);
		}
		if($type === 'string'){
			return self::string((string)$value);
		}
		if($type === 'byteArray'){
			if(is_array($value)){
				$bytes = '';
				foreach($value as $byte){
					$bytes .= chr(((int)$byte) & 0xFF);
				}
			}else{
				$bytes = (string)$value;
			}
			return pack('N', strlen($bytes)) . $bytes;
		}
		if($type === 'intArray'){
			$out = pack('N', count((array)$value));
			foreach((array)$value as $number){
				$out .= pack('N', ((int)$number) & 0xFFFFFFFF);
			}
			return $out;
		}
		if($type === 'longArray'){
			$out = pack('N', count((array)$value));
			foreach((array)$value as $number){
				$out .= pack('J', self::longValue($number));
			}
			return $out;
		}

		throw new \InvalidArgumentException("Tipo NBT desconocido: {$type}");
	}

	private static function string(string $value) : string{
		return pack('n', strlen($value)) . $value;
	}

	/** @param int|array{0:int,1:int} $value */
	private static function longValue(mixed $value) : int{
		if(is_array($value)){
			$high = (int)($value[0] ?? 0);
			$low = (int)($value[1] ?? 0);
			return (($high & 0xFFFFFFFF) << 32) | ($low & 0xFFFFFFFF);
		}
		return (int)$value;
	}
}
