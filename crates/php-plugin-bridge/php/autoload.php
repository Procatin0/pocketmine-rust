<?php
declare(strict_types=1);

namespace pocketmine\runtime;

/**
 * Per-plugin PSR-4 autoloader. Supports normal directories and phar:// roots.
 */
final class PluginAutoloader
{
    /** @var array<string, array{root:string, prefix:string}> */
    private static array $plugins = [];

    /** @var array<string, bool> */
    private static array $vendorLoaded = [];

    private static bool $registered = false;

    public static function register(): void
    {
        if (!self::$registered) {
            spl_autoload_register([self::class, 'autoload'], true, false);
            self::$registered = true;
        }
    }

    /**
     * @param string $root Plugin root directory, or a phar:// URI to the .phar archive.
     */
    public static function registerPlugin(string $name, string $root, string $mainClass): void
    {
        self::register();

        $root = self::normalizeRoot($root);
        $mainClass = ltrim(str_replace('/', '\\', $mainClass), '\\');
        $pos = strrpos($mainClass, '\\');
        $prefix = $pos === false ? '' : substr($mainClass, 0, $pos + 1);

        self::$plugins[$name] = [
            'root' => $root,
            'prefix' => $prefix,
        ];
        self::$vendorLoaded[$name] = false;

        // Composer dependencies are usually needed while the main class is parsed.
        self::loadVendor($name);
    }

    public static function unregisterPlugin(string $name): void
    {
        if (isset(self::$plugins[$name])) {
            unset(self::$plugins[$name], self::$vendorLoaded[$name]);
            return;
        }
        foreach (array_keys(self::$plugins) as $registered) {
            if (strcasecmp($registered, $name) === 0) {
                unset(self::$plugins[$registered], self::$vendorLoaded[$registered]);
                return;
            }
        }
    }

    public static function isRegistered(string $name): bool
    {
        if (isset(self::$plugins[$name])) {
            return true;
        }
        foreach (array_keys(self::$plugins) as $registered) {
            if (strcasecmp($registered, $name) === 0) {
                return true;
            }
        }
        return false;
    }

    /** @return array<string, array{root:string, prefix:string}> */
    public static function getPrefixes(): array
    {
        return self::$plugins;
    }

    public static function autoload(string $class): void
    {
        $class = ltrim($class, '\\');
        if ($class === '') {
            return;
        }

        $relative = str_replace('\\', '/', $class) . '.php';

        // 1. Prefix-specific PSR-4 lookup.
        foreach (self::$plugins as $plugin) {
            $prefix = $plugin['prefix'];
            if ($prefix !== '' && stripos($class, $prefix) === 0) {
                $inner = substr($class, strlen($prefix));
                $candidate = $plugin['root'] . '/src/' . str_replace('\\', '/', $inner) . '.php';
                if (self::loadFile($candidate)) {
                    return;
                }
            }
        }

        // 2. Fallback: the full class name relative to every plugin src/ directory.
        foreach (self::$plugins as $plugin) {
            if (self::loadFile($plugin['root'] . '/src/' . $relative)) {
                return;
            }
        }

        // 3. Composer autoloaders, at most once per plugin.
        foreach (array_keys(self::$plugins) as $name) {
            self::loadVendor($name);
        }
    }

    /**
     * Resolves an exact path, falling back to a case-insensitive segment walk.
     * Works for normal files and phar:// paths.
     */
    public static function resolve(string $path): ?string
    {
        $path = self::normalizePath($path);
        if ($path === '') {
            return null;
        }
        if (@is_file($path)) {
            return $path;
        }
        return self::resolveCaseInsensitive($path);
    }

    public static function fileExists(string $path): bool
    {
        return self::resolve($path) !== null;
    }

    private static function loadFile(string $path): bool
    {
        $resolved = self::resolve($path);
        if ($resolved === null) {
            return false;
        }
        require_once $resolved;
        return true;
    }

    private static function loadVendor(string $name): void
    {
        if (!isset(self::$plugins[$name]) || !empty(self::$vendorLoaded[$name])) {
            return;
        }
        self::$vendorLoaded[$name] = true;
        $vendor = self::$plugins[$name]['root'] . '/vendor/autoload.php';
        $resolved = self::resolve($vendor);
        if ($resolved === null) {
            return;
        }
        try {
            require_once $resolved;
        } catch (\Throwable $e) {
            self::log('Failed to load vendor autoload for ' . $name . ': ' . $e->getMessage());
        }
    }

    private static function normalizeRoot(string $root): string
    {
        $root = self::normalizePath($root);
        if (!self::isPhar($root)) {
            $real = @realpath($root);
            if ($real !== false) {
                $root = self::normalizePath($real);
            }
        }
        $root = rtrim($root, '/');
        if ($root === '') {
            $root = '.';
        }
        if (preg_match('#^[A-Za-z]:$#', $root) === 1) {
            $root .= '/';
        }
        return $root;
    }

    private static function isPhar(string $path): bool
    {
        return stripos($path, 'phar://') === 0;
    }

    private static function normalizePath(string $path): string
    {
        $scheme = '';
        if (stripos($path, 'phar://') === 0) {
            $scheme = 'phar://';
            $path = substr($path, 7);
        }

        $path = str_replace('\\', '/', $path);

        $leading = '';
        if (str_starts_with($path, '//')) {
            $leading = '//';
            $path = substr($path, 2);
        } elseif (str_starts_with($path, '/')) {
            $leading = '/';
            $path = substr($path, 1);
        }
        $path = preg_replace('#/+#', '/', $path) ?? $path;

        return $scheme . $leading . $path;
    }

    private static function resolveCaseInsensitive(string $path): ?string
    {
        if (self::isPhar($path)) {
            $rest = substr($path, 7);
            $offset = 0;
            $archive = null;
            $inner = '';
            while (($pos = stripos($rest, '.phar', $offset)) !== false) {
                $after = $pos + 5;
                if ($after === strlen($rest) || $rest[$after] === '/' || $rest[$after] === '\\') {
                    $archive = substr($rest, 0, $after);
                    $inner = ltrim(substr($rest, $after), '/\\');
                    break;
                }
                $offset = $after;
            }
            if ($archive === null) {
                return null;
            }
            $resolvedArchive = self::resolveFsPath($archive);
            if ($resolvedArchive === null) {
                return null;
            }
            $current = 'phar://' . $resolvedArchive;
            if ($inner === '') {
                return @is_file($current) ? $current : null;
            }
            foreach (explode('/', str_replace('\\', '/', $inner)) as $segment) {
                if ($segment === '') {
                    continue;
                }
                $next = self::findEntry($current, $segment);
                if ($next === null) {
                    return null;
                }
                $current = $next;
            }
            return @is_file($current) ? $current : null;
        }

        return self::resolveFsPath($path);
    }

    private static function resolveFsPath(string $path): ?string
    {
        if (@is_file($path)) {
            return $path;
        }

        $root = '';
        $rest = $path;

        if (preg_match('#^([A-Za-z]:)(/.*)?$#', $path, $m) === 1) {
            $root = $m[1] . '/';
            $rest = ltrim($m[2] ?? '', '/');
        } elseif (str_starts_with($path, '//')) {
            $parts = explode('/', substr($path, 2));
            $server = array_shift($parts) ?? '';
            $share = array_shift($parts) ?? '';
            if ($server !== '' && $share !== '') {
                $root = '//' . $server . '/' . $share . '/';
                $rest = implode('/', $parts);
            }
        } elseif (str_starts_with($path, '/')) {
            $root = '/';
            $rest = substr($path, 1);
        }

        $current = $root;
        foreach (explode('/', $rest) as $segment) {
            if ($segment === '') {
                continue;
            }
            $dir = $current === '' ? '.' : $current;
            $found = self::findEntry($dir, $segment);
            if ($found === null) {
                return null;
            }
            $current = $found;
        }

        return ($current !== '' && @is_file($current)) ? $current : null;
    }

    private static function findEntry(string $dir, string $segment): ?string
    {
        $base = rtrim($dir, '/');
        $candidate = $base . '/' . $segment;
        if (@file_exists($candidate)) {
            return $candidate;
        }

        $entries = @scandir($dir);
        if ($entries === false) {
            return null;
        }
        $needle = strtolower($segment);
        foreach ($entries as $entry) {
            if (strtolower($entry) === $needle) {
                return $base . '/' . $entry;
            }
        }
        return null;
    }

    private static function log(string $message): void
    {
        fwrite(STDERR, '[PHP/RUNTIME] ' . $message . PHP_EOL);
    }
}

/**
 * Public facade required by the runtime/plugin API.
 */
final class Autoloader
{
    public static function register(): void
    {
        PluginAutoloader::register();
    }

    public static function registerPlugin(string $name, string $root, string $mainClass): void
    {
        PluginAutoloader::registerPlugin($name, $root, $mainClass);
    }

    public static function unregisterPlugin(string $name): void
    {
        PluginAutoloader::unregisterPlugin($name);
    }

    public static function isRegistered(string $name): bool
    {
        return PluginAutoloader::isRegistered($name);
    }

    /** @return array<string, array{root:string, prefix:string}> */
    public static function getPrefixes(): array
    {
        return PluginAutoloader::getPrefixes();
    }

    public static function autoload(string $class): void
    {
        PluginAutoloader::autoload($class);
    }

    public static function resolve(string $path): ?string
    {
        return PluginAutoloader::resolve($path);
    }
}
