<?php

declare(strict_types=1);

namespace pocketmine\utils {

    final class TextFormat
    {
        public const ESCAPE = "\xc2\xa7";

        public const BLACK = self::ESCAPE . '0';
        public const DARK_BLUE = self::ESCAPE . '1';
        public const DARK_GREEN = self::ESCAPE . '2';
        public const DARK_AQUA = self::ESCAPE . '3';
        public const DARK_RED = self::ESCAPE . '4';
        public const DARK_PURPLE = self::ESCAPE . '5';
        public const GOLD = self::ESCAPE . '6';
        public const GRAY = self::ESCAPE . '7';
        public const DARK_GRAY = self::ESCAPE . '8';
        public const BLUE = self::ESCAPE . '9';
        public const GREEN = self::ESCAPE . 'a';
        public const AQUA = self::ESCAPE . 'b';
        public const RED = self::ESCAPE . 'c';
        public const LIGHT_PURPLE = self::ESCAPE . 'd';
        public const YELLOW = self::ESCAPE . 'e';
        public const WHITE = self::ESCAPE . 'f';
        public const MINECOIN_GOLD = self::ESCAPE . 'g';

        public const MATERIAL_QUARTZ = self::ESCAPE . 'h';
        public const MATERIAL_IRON = self::ESCAPE . 'i';
        public const MATERIAL_NETHERITE = self::ESCAPE . 'j';
        public const MATERIAL_REDSTONE = self::ESCAPE . 'm';
        public const MATERIAL_COPPER = self::ESCAPE . 'n';
        public const MATERIAL_GOLD = self::ESCAPE . 'p';
        public const MATERIAL_EMERALD = self::ESCAPE . 'q';
        public const MATERIAL_DIAMOND = self::ESCAPE . 's';
        public const MATERIAL_LAPIS = self::ESCAPE . 't';
        public const MATERIAL_AMETHYST = self::ESCAPE . 'u';
        public const MATERIAL_RESIN = self::ESCAPE . 'v';

        public const OBFUSCATED = self::ESCAPE . 'k';
        public const BOLD = self::ESCAPE . 'l';
        public const STRIKETHROUGH = self::ESCAPE . 'm';
        public const UNDERLINE = self::ESCAPE . 'n';
        public const ITALIC = self::ESCAPE . 'o';
        public const RESET = self::ESCAPE . 'r';

        /** @var array<string, string> */
        private static array $formatMap = [
            '0' => 'black',
            '1' => 'dark_blue',
            '2' => 'dark_green',
            '3' => 'dark_aqua',
            '4' => 'dark_red',
            '5' => 'dark_purple',
            '6' => 'gold',
            '7' => 'gray',
            '8' => 'dark_gray',
            '9' => 'blue',
            'a' => 'green',
            'b' => 'aqua',
            'c' => 'red',
            'd' => 'light_purple',
            'e' => 'yellow',
            'f' => 'white',
            'g' => 'minecoin_gold',
            'h' => 'material_quartz',
            'i' => 'material_iron',
            'j' => 'material_netherite',
            'k' => 'obfuscated',
            'l' => 'bold',
            'm' => 'strikethrough',
            'n' => 'underline',
            'o' => 'italic',
            'r' => 'reset',
        ];

        private static string $colorCodes = '0123456789abcdefghijklmnopqrstuv';

        public static function colorize(string $string): string
        {
            return preg_replace('/&([0-9a-v])/i', self::ESCAPE . '$1', $string) ?? $string;
        }

        public static function clean(string $string): string
        {
            return self::stripColors($string);
        }

        public static function stripColors(string $string): string
        {
            return preg_replace('/' . preg_quote(self::ESCAPE, '/') . '[0-9a-v]/i', '', $string) ?? $string;
        }

        public static function toANSI(string $string): string
        {
            $colors = [
                '0' => "\033[30m", '1' => "\033[34m", '2' => "\033[32m", '3' => "\033[36m",
                '4' => "\033[31m", '5' => "\033[35m", '6' => "\033[33m", '7' => "\033[37m",
                '8' => "\033[90m", '9' => "\033[94m", 'a' => "\033[92m", 'b' => "\033[96m",
                'c' => "\033[91m", 'd' => "\033[95m", 'e' => "\033[93m", 'f' => "\033[97m",
                'l' => "\033[1m", 'm' => "\033[9m", 'n' => "\033[4m", 'o' => "\033[3m",
                'r' => "\033[0m",
            ];
            $out = '';
            $length = strlen($string);
            for ($i = 0; $i < $length; $i++) {
                $char = $string[$i];
                if ($char === "\xc2" && $i + 1 < $length && $string[$i + 1] === "\xa7" && $i + 2 < $length) {
                    $code = strtolower($string[$i + 2]);
                    if (isset($colors[$code])) {
                        $out .= $colors[$code];
                    }
                    $i += 2;
                    continue;
                }
                $out .= $char;
            }
            return $out;
        }

        public static function getByChar(string $char): ?string
        {
            $char = strtolower($char);
            return self::$formatMap[$char] ?? null;
        }

        public static function getByName(string $name): ?string
        {
            $name = strtolower($name);
            $key = array_search($name, self::$formatMap, true);
            return $key === false ? null : self::ESCAPE . $key;
        }

        public static function addFormat(string $string, string $format): string
        {
            return $string . $format;
        }

        public static function tokenize(string $string): array
        {
            return preg_split('/(' . preg_quote(self::ESCAPE, '/') . '[0-9a-v])/i', $string, -1, PREG_SPLIT_DELIM_CAPTURE | PREG_SPLIT_NO_EMPTY) ?: [];
        }

        public static function toHTML(string $string): string
        {
            return htmlspecialchars(self::stripColors($string), ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
        }
    }

    class Utils
    {
        public static function getRandomFloat(float $min = 0.0, float $max = 1.0): float
        {
            return $min + ($max - $min) * (mt_rand() / mt_getrandmax());
        }

        public static function getRandomInt(int $min, int $max): int
        {
            return mt_rand($min, $max);
        }

        public static function getCurrentTicks(): int
        {
            return (int)(hrtime(true) / 1_000_000);
        }

        public static function getNiceTickCount(): int
        {
            return max(1, self::getCurrentTicks());
        }

        public static function getNiceClassName(string $class): string
        {
            $pos = strrpos($class, '\\');
            return $pos === false ? $class : substr($class, $pos + 1);
        }

        public static function validateArgumentCount(int $count, int $min, ?int $max = null): bool
        {
            return $count >= $min && ($max === null || $count <= $max);
        }

        public static function assocToIndexed(array $array): array
        {
            $result = [];
            foreach ($array as $value) {
                $result[] = $value;
            }
            return $result;
        }
    }

    /**
     * PM `pocketmine\utils\SingletonTrait`: shared static instance holder
     * used by virions (ScoreFactory) and plugins (ScoreHud). Only the
     * instance storage/accessors are ported — no private constructor, so
     * the bridge plugin loader can still `new` the using class (PM
     * instantiates plugins itself, the trait never constructs them).
     */
    trait SingletonTrait
    {
        private static ?self $instance = null;

        public static function getInstance() : self
        {
            return self::$instance ??= new self();
        }

        public static function setInstance(?self $instance) : void
        {
            self::$instance = $instance;
        }

        public function __clone()
        {
            throw new \LogicException("Cloning a singleton is not allowed");
        }

        public function __wakeup()
        {
            throw new \LogicException("Unserializing a singleton is not allowed");
        }
    }

    class ServerException extends \RuntimeException
    {
    }

    class ConfigException extends ServerException
    {
    }
}
