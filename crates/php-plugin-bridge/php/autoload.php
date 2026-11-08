<?php
declare(strict_types=1);

namespace pocketmine\runtime;

/**
 * Per-plugin PSR-4 autoloader. Supports normal directories and phar:// roots.
 *
 * PM-parity notes (see `pocketmine\thread\ThreadSafeClassLoader`):
 * `registerPlugin()` maps the plugin's effective src-namespace prefix to
 * its `src/` directory — for `.phar` plugins that is `phar://<file>/src`,
 * the equivalent of `PharPluginLoader::loadPlugin()` calling
 * `$loader->addPath($prefix, "$file/src")`. Entries are isolated by
 * `description name + plugin root`, so two `.phar` archives never overwrite
 * each other's registration (the archive may be named differently from the
 * plugin inside it).
 *
 * This class *is* the `ThreadSafeClassLoader` equivalent: like
 * `ThreadSafeClassLoader::register()`, it registers a single
 * `spl_autoload_register` callback serving every plugin. When PHP's Phar
 * support is available the callback resolves `phar://` paths directly;
 * without it, phar entries simply never resolve (the loader logs a clear
 * message instead of crashing).
 */
final class PluginAutoloader
{
    /**
     * Isolation key => entry. The key combines the case-insensitive
     * description name with the plugin root (for phar archives the full
     * `phar://<path>`), so distinct archives never collide.
     *
     * @var array<string, array{name:string, root:string, prefix:string, srcPrefix:string, mainNs:string, srcDir:string}>
     */
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
     * Registers a plugin source.
     *
     * @param string $name Description name (`plugin.yml` `name:`).
     * @param string $root Plugin root directory, or a phar:// URI to the .phar archive.
     * @param string $mainClass FQCN of the `main:` class.
     * @param string|null $srcPrefix `src-namespace-prefix` from the manifest,
     *     or null/'' to fall back to the namespace of `$mainClass`.
     */
    public static function registerPlugin(string $name, string $root, string $mainClass, ?string $srcPrefix = null): void
    {
        self::register();

        $root = self::normalizeRoot($root);
        $mainClass = ltrim(str_replace('/', '\\', $mainClass), '\\');
        $mainNs = self::mainNamespace($mainClass);
        $explicit = self::normalizePrefix($srcPrefix ?? '');
        // Effective prefix (PM `addPath($prefix, "<root>/src")` equivalent):
        // the manifest prefix wins, otherwise the namespace of `main`.
        $prefix = $explicit !== '' ? $explicit : ($mainNs !== '' ? $mainNs . '\\' : '');

        $key = self::isolationKey($name, $root);
        self::$plugins[$key] = [
            'name' => $name,
            'root' => $root,
            'prefix' => $prefix,
            'srcPrefix' => $explicit,
            'mainNs' => $mainNs,
            'srcDir' => $root . '/src',
        ];
        self::$vendorLoaded[$key] = false;

        // Composer dependencies are usually needed while the main class is parsed.
        self::loadVendor($key);
    }

    /**
     * PM `ThreadSafeClassLoader::addPath()` equivalent: maps a namespace
     * prefix to an explicit `src/` directory (e.g. `phar://<file>/src`).
     * Prefer {@see registerPlugin} for manifest-based plugins; this exists
     * for PM-parity standalone use.
     */
    public static function addPath(string $namespacePrefix, string $path): void
    {
        self::register();

        $prefix = self::normalizePrefix($namespacePrefix);
        $srcDir = self::normalizePath(rtrim(str_replace('\\', '/', $path), '/'));
        if ($srcDir === '') {
            return;
        }
        $key = 'path:' . $prefix . "\0" . $srcDir;
        self::$plugins[$key] = [
            'name' => '',
            'root' => '',
            'prefix' => $prefix,
            'srcPrefix' => $prefix,
            'mainNs' => '',
            'srcDir' => $srcDir,
        ];
        self::$vendorLoaded[$key] = false;

        if ($prefix !== '') {
            self::loadVendor($key);
        }
    }

    public static function unregisterPlugin(string $name): void
    {
        if (isset(self::$plugins[$name])) {
            unset(self::$plugins[$name], self::$vendorLoaded[$name]);
            return;
        }
        // Plain description name: remove every isolated entry with that name.
        foreach (array_keys(self::$plugins) as $registered) {
            if (strcasecmp(self::$plugins[$registered]['name'], $name) === 0) {
                unset(self::$plugins[$registered], self::$vendorLoaded[$registered]);
            }
        }
    }

    public static function isRegistered(string $name): bool
    {
        if (isset(self::$plugins[$name])) {
            return true;
        }
        foreach (self::$plugins as $registered) {
            if (strcasecmp($registered['name'], $name) === 0) {
                return true;
            }
        }
        return false;
    }

    /**
     * @return array<string, array{name:string, root:string, prefix:string, srcPrefix:string, mainNs:string, srcDir:string}>
     */
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

        // 1. Prefix-specific PSR-4 lookup (PM `ThreadSafeClassLoader`):
        // strip the effective src-namespace prefix, resolve below `src/`.
        // This covers `src-namespace-prefix` layouts (`BetterChat\Main` ->
        // `src/Main.php`) as well as the namespace-of-`main` default.
        foreach (self::$plugins as $plugin) {
            $prefix = $plugin['prefix'];
            if ($prefix !== '' && stripos($class, $prefix) === 0) {
                $inner = substr($class, strlen($prefix));
                $candidate = $plugin['srcDir'] . '/' . str_replace('\\', '/', $inner) . '.php';
                if (self::loadFile($candidate)) {
                    return;
                }
            }
        }

        // 2. Fallback: the full class name relative to every plugin src/ directory.
        foreach (self::$plugins as $plugin) {
            if (self::loadFile($plugin['srcDir'] . '/' . $relative)) {
                return;
            }
        }

        // 3. Legacy: strip the *main* namespace even when a manifest prefix
        // was set (bridge plugins that used the derived-prefix layout).
        foreach (self::$plugins as $plugin) {
            $mainNs = $plugin['mainNs'];
            if ($mainNs !== '' && $plugin['prefix'] !== $mainNs . '\\'
                && stripos($class, $mainNs . '\\') === 0
            ) {
                $inner = substr($class, strlen($mainNs) + 1);
                $candidate = $plugin['srcDir'] . '/' . str_replace('\\', '/', $inner) . '.php';
                if (self::loadFile($candidate)) {
                    return;
                }
            }
        }

        // 4. Composer autoloaders, at most once per plugin.
        foreach (array_keys(self::$plugins) as $key) {
            self::loadVendor($key);
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

    private static function loadVendor(string $key): void
    {
        $key = self::resolveKey($key);
        if ($key === null || !empty(self::$vendorLoaded[$key])) {
            return;
        }
        self::$vendorLoaded[$key] = true;
        $root = self::$plugins[$key]['root'];
        if ($root === '') {
            return;
        }
        $vendor = $root . '/vendor/autoload.php';
        $resolved = self::resolve($vendor);
        if ($resolved === null) {
            return;
        }
        try {
            require_once $resolved;
        } catch (\Throwable $e) {
            self::log('Failed to load vendor autoload for ' . $key . ': ' . $e->getMessage());
        }
    }

    /**
     * Isolation key for one plugin source: case-insensitive description name
     * plus the plugin root. Two `.phar` files (or a dir and a phar) can never
     * share a key, even when their description names collide.
     */
    private static function isolationKey(string $name, string $root): string
    {
        return strtolower(trim($name)) . "\0" . $root;
    }

    /** Accepts an isolation key or a plain description name; returns the key. */
    private static function resolveKey(string $name): ?string
    {
        if (isset(self::$plugins[$name])) {
            return $name;
        }
        foreach (self::$plugins as $key => $plugin) {
            if (strcasecmp($plugin['name'], $name) === 0) {
                return $key;
            }
        }
        return null;
    }

    /**
     * Normalizes `src-namespace-prefix` to `Foo\Bar\` form (trailing
     * backslash), or `''` when unset — the PM `addPath()` prefix shape.
     */
    private static function normalizePrefix(string $prefix): string
    {
        $prefix = trim(str_replace('/', '\\', $prefix), '\\');
        if ($prefix === '') {
            return '';
        }
        return $prefix . '\\';
    }

    /** Namespace of a FQCN (`Acme\Foo\Main` -> `Acme\Foo`), or `''`. */
    private static function mainNamespace(string $mainClass): string
    {
        $mainClass = ltrim(str_replace('/', '\\', $mainClass), '\\');
        $pos = strrpos($mainClass, '\\');
        return $pos === false ? '' : substr($mainClass, 0, $pos);
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

    public static function registerPlugin(string $name, string $root, string $mainClass, ?string $srcPrefix = null): void
    {
        PluginAutoloader::registerPlugin($name, $root, $mainClass, $srcPrefix);
    }

    public static function addPath(string $namespacePrefix, string $path): void
    {
        PluginAutoloader::addPath($namespacePrefix, $path);
    }

    public static function unregisterPlugin(string $name): void
    {
        PluginAutoloader::unregisterPlugin($name);
    }

    public static function isRegistered(string $name): bool
    {
        return PluginAutoloader::isRegistered($name);
    }

    /** @return array<string, array{name:string, root:string, prefix:string, srcPrefix:string, mainNs:string, srcDir:string}> */
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
