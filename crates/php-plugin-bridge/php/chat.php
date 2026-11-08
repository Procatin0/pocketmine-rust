<?php

declare(strict_types=1);

namespace pocketmine\player\chat {

    /**
     * PM `pocketmine\player\chat\ChatFormatter` parity.
     *
     * A formatter renders the broadcast line of a `PlayerChatEvent` from
     * the sender's username and the (already filtered) message. Plugins
     * (BetterChat) implement it; the event carries it via
     * `setFormatter()` / `getFormatter()` and the bridge renders `format`
     * through it before serialising to Rust.
     */
    interface ChatFormatter
    {
        public function format(string $username, string $message): string;
    }

    /**
     * Placeholder formatter with the same semantics as BetterChat's
     * `ChatFormatter`: `{key}` entries in `$format` are replaced with
     * `$placeholders`, plus the implicit `{player}` / `{message}`.
     *
     * Provided as a convenience for plugins without their own formatter;
     * kept identical to `PlaceholderFormatter` on the Rust side
     * (`php-plugin-bridge/src/chat.rs`).
     */
    class PlaceholderFormatter implements ChatFormatter
    {
        /**
         * @param array<string, string> $placeholders
         */
        public function __construct(
            private string $format,
            private array $placeholders = []
        ) {
        }

        public function format(string $username, string $message): string
        {
            $replacements = array_merge($this->placeholders, [
                "player" => $username,
                "message" => $message,
            ]);
            $result = $this->format;
            foreach ($replacements as $key => $value) {
                $result = str_replace("{" . $key . "}", (string)$value, $result);
            }
            return $result;
        }

        public function getFormat(): string
        {
            return $this->format;
        }

        /** @return array<string, string> */
        public function getPlaceholders(): array
        {
            return $this->placeholders;
        }
    }
}
