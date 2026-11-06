<?php

declare(strict_types=1);

namespace pocketmine\utils {

    use pocketmine\utils\ConfigException;

    class Config
    {
        public const YAML = 2;
        public const JSON = 4;
        public const PROPERTIES = 8;
        public const ENUM = 16;
        public const DETECT = 32;

        /** @var array<string, mixed> */
        private array $data = [];

        private int $type;

        public function __construct(
            private string $path,
            int $type = self::DETECT,
            array $default = []
        ) {
            $this->type = $this->resolveType($path, $type);
            $this->path = $this->normalizePath($path);

            if ($this->path !== '' && is_file($this->path)) {
                $this->load();
            } else {
                $this->data = $default;
            }
        }

        private function normalizePath(string $path): string
        {
            if ($path === '') {
                return '';
            }
            if (!str_contains($path, '.') && $path !== '') {
                $path .= '.yml';
            }
            return $path;
        }

        private function resolveType(string $path, int $type): int
        {
            if ($type !== self::DETECT) {
                return $type;
            }
            $extension = strtolower(pathinfo($path, PATHINFO_EXTENSION));
            return match ($extension) {
                'json' => self::JSON,
                'properties' => self::PROPERTIES,
                'yaml', 'yml' => self::YAML,
                default => self::YAML,
            };
        }

        public function load(): void
        {
            $this->data = [];
            if ($this->path === '' || !is_file($this->path)) {
                return;
            }
            $contents = @file_get_contents($this->path);
            if ($contents === false || trim($contents) === '') {
                return;
            }

            $parsed = match ($this->type) {
                self::JSON => json_decode($contents, true),
                self::PROPERTIES => $this->parseProperties($contents),
                default => $this->parseYaml($contents),
            };

            $this->data = is_array($parsed) ? $parsed : [];
        }

        public function reload(): void
        {
            $this->load();
        }

        public function save(): void
        {
            if ($this->path === '') {
                throw new ConfigException('Cannot save a configuration without a path');
            }
            $dir = dirname($this->path);
            if ($dir !== '' && !is_dir($dir)) {
                @mkdir($dir, 0777, true);
            }

            $contents = match ($this->type) {
                self::JSON => json_encode($this->data, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
                self::PROPERTIES => $this->emitProperties($this->data),
                default => $this->emitYaml($this->data),
            };
            if ($contents === false) {
                throw new ConfigException('Failed to serialize configuration data');
            }

            $tmp = $this->path . '.tmp' . bin2hex(random_bytes(4));
            if (@file_put_contents($tmp, $contents, LOCK_EX) === false) {
                throw new ConfigException('Unable to write configuration to ' . $tmp);
            }
            if (!@rename($tmp, $this->path)) {
                @unlink($this->path);
                if (!@rename($tmp, $this->path)) {
                    @unlink($tmp);
                    throw new ConfigException('Unable to replace configuration at ' . $this->path);
                }
            }
        }

        public function get(string $key, mixed $default = null): mixed
        {
            $current = $this->data;
            foreach (explode('.', $key) as $part) {
                if (!is_array($current) || !array_key_exists($part, $current)) {
                    return $default;
                }
                $current = $current[$part];
            }
            return $current;
        }

        public function getNested(string $key, mixed $default = null): mixed
        {
            return $this->get($key, $default);
        }

        public function set(string $key, mixed $value): void
        {
            $parts = explode('.', $key);
            $current = &$this->data;
            foreach ($parts as $index => $part) {
                if ($index === count($parts) - 1) {
                    $current[$part] = $value;
                    return;
                }
                if (!isset($current[$part]) || !is_array($current[$part])) {
                    $current[$part] = [];
                }
                $current = &$current[$part];
            }
        }

        public function setNested(string $key, mixed $value): void
        {
            $this->set($key, $value);
        }

        public function remove(string $key): void
        {
            $parts = explode('.', $key);
            $last = array_pop($parts);
            $current = &$this->data;
            foreach ($parts as $part) {
                if (!isset($current[$part]) || !is_array($current[$part])) {
                    return;
                }
                $current = &$current[$part];
            }
            unset($current[$last]);
        }

        public function removeNested(string $key): void
        {
            $this->remove($key);
        }

        public function exists(string $key): bool
        {
            $current = $this->data;
            foreach (explode('.', $key) as $part) {
                if (!is_array($current) || !array_key_exists($part, $current)) {
                    return false;
                }
                $current = $current[$part];
            }
            return true;
        }

        public function setDefaults(array $defaults): void
        {
            $this->data = $this->mergeDefaults($this->data, $defaults);
        }

        private function mergeDefaults(array $actual, array $defaults): array
        {
            foreach ($defaults as $key => $value) {
                if (!array_key_exists($key, $actual)) {
                    $actual[$key] = $value;
                } elseif (is_array($value) && is_array($actual[$key])) {
                    $actual[$key] = $this->mergeDefaults($actual[$key], $value);
                }
            }
            return $actual;
        }

        public function getAll(bool $keys = false): array
        {
            return $keys ? $this->data : array_values($this->data);
        }

        public function setAll(array $data): void
        {
            $this->data = $data;
        }

        public function getPath(): string
        {
            return $this->path;
        }

        public function getType(): int
        {
            return $this->type;
        }

        private function parseYaml(string $contents): mixed
        {
            if (function_exists('yaml_parse')) {
                $parsed = @yaml_parse($contents);
                if ($parsed !== null) {
                    return $parsed;
                }
            }
            $decoded = json_decode($contents, true);
            if (is_array($decoded)) {
                return $decoded;
            }
            return $this->parseSimpleYaml($contents);
        }

        private function emitYaml(mixed $data): string
        {
            if (function_exists('yaml_emit')) {
                $emitted = @yaml_emit($data, YAML_UTF8_ENCODING);
                if (is_string($emitted)) {
                    return $emitted;
                }
            }
            $encoded = json_encode($data, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
            return $encoded === false ? '{}' : $encoded;
        }

        /**
         * Fallback for hosts without the yaml extension: supports nested maps
         * and scalars for the subset used by plugin configs.
         */
        private function parseSimpleYaml(string $contents): array
        {
            $root = [];
            $stack = [['indent' => -1, 'ref' => &$root]];
            $lines = preg_split('/\r\n|\r|\n/', $contents) ?: [];
            foreach ($lines as $line) {
                if (trim($line) === '' || ltrim($line)[0] === '#') {
                    continue;
                }
                $indent = strlen($line) - strlen(ltrim($line, ' '));
                $line = trim($line);
                if (!str_contains($line, ':')) {
                    continue;
                }
                [$key, $value] = explode(':', $line, 2);
                $key = trim($key);
                $value = trim($value);
                while (count($stack) > 1 && $indent <= $stack[count($stack) - 1]['indent']) {
                    array_pop($stack);
                }
                $ref = &$stack[count($stack) - 1]['ref'];
                if ($value === '') {
                    $ref[$key] = [];
                    $stack[] = ['indent' => $indent, 'ref' => &$ref[$key]];
                } else {
                    $ref[$key] = $this->parseScalar($value);
                }
                unset($ref);
            }
            return $root;
        }

        private function parseScalar(string $value): mixed
        {
            $value = trim($value, "\"'");
            if ($value === 'true') {
                return true;
            }
            if ($value === 'false') {
                return false;
            }
            if ($value === 'null' || $value === '~') {
                return null;
            }
            if (is_numeric($value)) {
                return str_contains($value, '.') ? (float)$value : (int)$value;
            }
            if (str_starts_with($value, '[') && str_ends_with($value, ']')) {
                $inner = trim(substr($value, 1, -1));
                return $inner === '' ? [] : array_map(fn(string $v) => $this->parseScalar(trim($v)), explode(',', $inner));
            }
            return $value;
        }

        private function parseProperties(string $contents): array
        {
            $result = [];
            foreach (preg_split('/\r\n|\r|\n/', $contents) ?: [] as $line) {
                $line = trim($line);
                if ($line === '' || $line[0] === '#' || $line[0] === '!') {
                    continue;
                }
                $pos = strpos($line, '=');
                if ($pos === false) {
                    continue;
                }
                $result[trim(substr($line, 0, $pos))] = $this->parseScalar(trim(substr($line, $pos + 1)));
            }
            return $result;
        }

        private function emitProperties(array $data): string
        {
            $out = '';
            foreach ($data as $key => $value) {
                if (is_array($value)) {
                    $value = json_encode($value);
                } elseif (is_bool($value)) {
                    $value = $value ? 'true' : 'false';
                }
                $out .= $key . '=' . $value . PHP_EOL;
            }
            return $out;
        }
    }
}
