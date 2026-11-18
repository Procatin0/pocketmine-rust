<?php

declare(strict_types=1);

namespace pocketmine\utils {

    /** PM `InternetException`: a request could not be completed. */
    class InternetException extends \RuntimeException
    {
    }

    /** PM `InternetRequestResult`: response headers, body and status code. */
    final class InternetRequestResult
    {
        /** @param list<array<string, string>> $headers */
        public function __construct(
            private array $headers,
            private string $body,
            private int $code
        ) {
        }

        /** @return list<array<string, string>> */
        public function getHeaders(): array
        {
            return $this->headers;
        }

        public function getBody(): string
        {
            return $this->body;
        }

        public function getCode(): int
        {
            return $this->code;
        }
    }

    /**
     * PM `Internet`: blocking HTTP helpers backed by ext-curl.
     *
     * EasyEdit calls these from its edit thread to download the block-state
     * repository, so the signatures (including the `$err` out parameter) and
     * the certificate handling match PM5: PM disables peer verification and
     * splits the raw response headers itself.
     */
    final class Internet
    {
        public static string|false $ip = false;
        public static bool $online = true;

        public static function getIP(bool $force = false): string|false
        {
            if (!self::$online) {
                return false;
            }
            if (self::$ip !== false && !$force) {
                return self::$ip;
            }
            $result = self::getURL('http://api.ipify.org/');
            if ($result !== null && trim($result->getBody()) !== '') {
                return self::$ip = trim($result->getBody());
            }
            return false;
        }

        /** @throws InternetException */
        public static function getInternalIP(): string
        {
            if (!function_exists('socket_create')) {
                throw new InternetException('ext-sockets is not loaded');
            }
            $socket = @socket_create(AF_INET, SOCK_DGRAM, SOL_UDP);
            if ($socket === false) {
                throw new InternetException('Failed to get internal IP: ' . trim((string)socket_strerror(socket_last_error())));
            }
            try {
                if (!@socket_connect($socket, '8.8.8.8', 65534)) {
                    throw new InternetException('Failed to get internal IP: ' . trim((string)socket_strerror(socket_last_error($socket))));
                }
                if (!@socket_getsockname($socket, $name)) {
                    throw new InternetException('Failed to get internal IP: ' . trim((string)socket_strerror(socket_last_error($socket))));
                }
                return $name;
            } finally {
                socket_close($socket);
            }
        }

        /** @param string[] $extraHeaders */
        public static function getURL(
            string $page,
            int $timeout = 10,
            array $extraHeaders = [],
            &$err = null
        ): ?InternetRequestResult {
            try {
                return self::simpleCurl($page, $timeout, $extraHeaders);
            } catch (InternetException $e) {
                $err = $e->getMessage();
                return null;
            }
        }

        /**
         * @param string[]|string $args
         * @param string[] $extraHeaders
         */
        public static function postURL(
            string $page,
            array|string $args,
            int $timeout = 10,
            array $extraHeaders = [],
            &$err = null
        ): ?InternetRequestResult {
            try {
                return self::simpleCurl($page, $timeout, $extraHeaders, [
                    CURLOPT_POST => 1,
                    CURLOPT_POSTFIELDS => $args,
                ]);
            } catch (InternetException $e) {
                $err = $e->getMessage();
                return null;
            }
        }

        /**
         * @param string[] $extraHeaders
         * @param array<int, mixed> $extraOpts
         * @throws InternetException
         */
        public static function simpleCurl(
            string $page,
            float $timeout = 10,
            array $extraHeaders = [],
            array $extraOpts = [],
            ?\Closure $onSuccess = null
        ): InternetRequestResult {
            if (!self::$online) {
                throw new InternetException('Cannot execute web request while offline');
            }
            if (!extension_loaded('curl')) {
                throw new InternetException('ext-curl is not loaded');
            }

            $curl = curl_init($page);
            if ($curl === false) {
                throw new InternetException('Unable to create new cURL session');
            }

            curl_setopt_array($curl, $extraOpts + [
                CURLOPT_SSL_VERIFYPEER => false,
                CURLOPT_SSL_VERIFYHOST => 2,
                CURLOPT_FORBID_REUSE => 1,
                CURLOPT_FRESH_CONNECT => 1,
                CURLOPT_AUTOREFERER => true,
                CURLOPT_FOLLOWLOCATION => true,
                CURLOPT_RETURNTRANSFER => true,
                CURLOPT_CONNECTTIMEOUT_MS => (int)($timeout * 1000),
                CURLOPT_TIMEOUT_MS => (int)($timeout * 1000),
                CURLOPT_HTTPHEADER => array_merge(
                    ['User-Agent: Mozilla/5.0 (Windows NT 6.1; WOW64; rv:12.0) Gecko/20100101 Firefox/12.0 PocketMine-MP/5.0.0'],
                    $extraHeaders
                ),
                CURLOPT_HEADER => true,
            ]);

            $raw = curl_exec($curl);
            if ($raw === false) {
                $error = curl_error($curl);
                curl_close($curl);
                throw new InternetException($error);
            }
            if (!is_string($raw)) {
                curl_close($curl);
                throw new InternetException('cURL did not return a response body');
            }

            $httpCode = (int)curl_getinfo($curl, CURLINFO_HTTP_CODE);
            $headerSize = (int)curl_getinfo($curl, CURLINFO_HEADER_SIZE);
            $rawHeaders = substr($raw, 0, $headerSize);
            $body = substr($raw, $headerSize);
            if ($onSuccess !== null) {
                $onSuccess($curl);
            }
            curl_close($curl);

            // PM returns one header map per response block (redirects included).
            $headers = [];
            foreach (explode("\r\n\r\n", $rawHeaders) as $rawHeaderGroup) {
                $headerGroup = [];
                foreach (explode("\r\n", $rawHeaderGroup) as $line) {
                    $nameValue = explode(':', $line, 2);
                    if (isset($nameValue[1])) {
                        $headerGroup[trim(strtolower($nameValue[0]))] = trim($nameValue[1]);
                    }
                }
                $headers[] = $headerGroup;
            }

            return new InternetRequestResult($headers, $body, $httpCode);
        }
    }
}
