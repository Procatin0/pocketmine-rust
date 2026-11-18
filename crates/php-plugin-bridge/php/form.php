<?php

declare(strict_types=1);

namespace pocketmine\form {
    use pocketmine\player\Player;

    /**
     * `pocketmine\form\Form` (PM5 core): a form the player can receive.
     *
     * The concrete forms (`SimpleForm`, `CustomForm`, `ModalForm` and the
     * element classes) live in the `pmmp/ext-forms` plugin upstream; they are
     * bundled here because the bridge has no plugin marketplace, and the wire
     * protocol is the same (`ModalFormRequestPacket` JSON + response).
     */
    interface Form extends \JsonSerializable
    {
        /**
         * Handles the player's response.
         *
         * @param mixed $data `null` when the player closed the form.
         * @throws FormValidationException when the data does not match the form.
         */
        public function handleResponse(Player $player, $data): void;
    }

    /** `pocketmine\form\FormValidationException`. */
    class FormValidationException extends \RuntimeException
    {
    }

    /** `pocketmine\form\Button` (`ext-forms`). */
    class Button implements \JsonSerializable
    {
        public function __construct(
            protected string $text,
            protected ?\Closure $onClick = null,
            protected ?string $imagePath = null
        ) {}

        public function getText(): string
        {
            return $this->text;
        }

        public function getImage(): ?string
        {
            return $this->imagePath;
        }

        public function handleClick(Player $player): void
        {
            if ($this->onClick !== null) {
                ($this->onClick)($player);
            }
        }

        public function jsonSerialize(): array
        {
            $data = ['text' => $this->text];
            if ($this->imagePath !== null) {
                $data['image'] = ['type' => 'path', 'data' => $this->imagePath];
            }
            return $data;
        }
    }

    /** `pocketmine\form\SimpleForm` (`ext-forms`). */
    class SimpleForm implements Form
    {
        /** @var Button[] */
        protected array $buttons = [];
        protected ?string $content;

        /**
         * @param \Closure(Player, ?int): void $onSubmit receives the pressed
         *        button index, or `null` when the form was closed.
         */
        public function __construct(
            protected string $title,
            protected \Closure $onSubmit,
            ?string $content = null
        ) {
            $this->content = $content;
        }

        public function getTitle(): string
        {
            return $this->title;
        }

        public function setContent(?string $content): void
        {
            $this->content = $content;
        }

        public function getContent(): ?string
        {
            return $this->content;
        }

        public function addButton(Button $button): void
        {
            $this->buttons[] = $button;
        }

        /** @return Button[] */
        public function getButtons(): array
        {
            return $this->buttons;
        }

        public function handleResponse(Player $player, $data): void
        {
            if ($data === null) {
                ($this->onSubmit)($player, null);
                return;
            }
            if (!is_int($data) && !(is_string($data) && ctype_digit($data))) {
                throw new FormValidationException('Expected integer button index, got ' . get_debug_type($data));
            }
            $index = (int)$data;
            if (!isset($this->buttons[$index])) {
                throw new FormValidationException("Button index $index is out of range");
            }
            $this->buttons[$index]->handleClick($player);
            ($this->onSubmit)($player, $index);
        }

        public function jsonSerialize(): array
        {
            $data = ['type' => 'form', 'title' => $this->title];
            if ($this->content !== null && $this->content !== '') {
                $data['content'] = $this->content;
            }
            $data['buttons'] = array_map(static fn(Button $button): array => $button->jsonSerialize(), $this->buttons);
            return $data;
        }
    }

    /** `pocketmine\form\FormElement` (`ext-forms`). */
    interface FormElement extends \JsonSerializable
    {
        /** Value sent back by the client for this element, or `null` if invalid. */
        public function getValue(mixed $data): mixed;
    }

    /** `pocketmine\form\Label`. */
    class Label implements FormElement
    {
        public function __construct(protected string $text) {}

        public function getValue(mixed $data): mixed
        {
            return null;
        }

        public function jsonSerialize(): array
        {
            return ['type' => 'label', 'text' => $this->text];
        }
    }

    /** `pocketmine\form\Input` (text field). */
    class Input implements FormElement
    {
        public function __construct(
            protected string $text,
            protected string $placeholder = '',
            protected string $default = ''
        ) {}

        public function getValue(mixed $data): ?string
        {
            if ($data === null) {
                return null;
            }
            return is_string($data) || is_numeric($data) ? (string)$data : null;
        }

        public function jsonSerialize(): array
        {
            return ['type' => 'input', 'text' => $this->text, 'placeholder' => $this->placeholder, 'default' => $this->default];
        }
    }

    /** `pocketmine\form\Dropdown`. */
    class Dropdown implements FormElement
    {
        /** @param string[] $options */
        public function __construct(
            protected string $text,
            protected array $options,
            protected int $default = 0
        ) {}

        public function getValue(mixed $data): ?int
        {
            if ($data === null) {
                return null;
            }
            if (!is_int($data) && !(is_string($data) && ctype_digit($data))) {
                return null;
            }
            $index = (int)$data;
            return isset($this->options[$index]) ? $index : null;
        }

        public function jsonSerialize(): array
        {
            return ['type' => 'dropdown', 'text' => $this->text, 'options' => array_values($this->options), 'default' => $this->default];
        }
    }

    /** `pocketmine\form\Slider`. */
    class Slider implements FormElement
    {
        public function __construct(
            protected string $text,
            protected float $min,
            protected float $max,
            protected float $step = 1.0,
            protected float $default = 0.0
        ) {}

        public function getValue(mixed $data): ?float
        {
            if (!is_int($data) && !is_float($data) && !(is_string($data) && is_numeric($data))) {
                return null;
            }
            $value = (float)$data;
            return $value >= $this->min && $value <= $this->max ? $value : null;
        }

        public function jsonSerialize(): array
        {
            return [
                'type' => 'slider',
                'text' => $this->text,
                'min' => $this->min,
                'max' => $this->max,
                'step' => $this->step,
                'default' => $this->default,
            ];
        }
    }

    /** `pocketmine\form\StepSlider`. */
    class StepSlider implements FormElement
    {
        /** @param string[] $steps */
        public function __construct(
            protected string $text,
            protected array $steps,
            protected int $default = 0
        ) {}

        public function getValue(mixed $data): ?int
        {
            if (!is_int($data) && !(is_string($data) && ctype_digit($data))) {
                return null;
            }
            $index = (int)$data;
            return isset($this->steps[$index]) ? $index : null;
        }

        public function jsonSerialize(): array
        {
            return ['type' => 'step_slider', 'text' => $this->text, 'steps' => array_values($this->steps), 'default' => $this->default];
        }
    }

    /** `pocketmine\form\Toggle` (switch). */
    class Toggle implements FormElement
    {
        public function __construct(
            protected string $text,
            protected bool $default = false
        ) {}

        public function getValue(mixed $data): ?bool
        {
            return is_bool($data) ? $data : null;
        }

        public function jsonSerialize(): array
        {
            return ['type' => 'toggle', 'text' => $this->text, 'default' => $this->default];
        }
    }

    /** `pocketmine\form\CustomForm` (`ext-forms`). */
    class CustomForm implements Form
    {
        /** @var FormElement[] */
        protected array $elements = [];

        /** @param \Closure(Player, ?array): void $onSubmit */
        public function __construct(
            protected string $title,
            protected \Closure $onSubmit
        ) {}

        public function getTitle(): string
        {
            return $this->title;
        }

        public function addElement(FormElement $element): void
        {
            $this->elements[] = $element;
        }

        public function addLabel(string $text): void
        {
            $this->addElement(new Label($text));
        }

        public function addInput(string $text, string $placeholder = '', string $default = ''): void
        {
            $this->addElement(new Input($text, $placeholder, $default));
        }

        /** @param string[] $options */
        public function addDropdown(string $text, array $options, int $default = 0): void
        {
            $this->addElement(new Dropdown($text, $options, $default));
        }

        public function addSlider(string $text, float $min, float $max, float $step = 1.0, float $default = 0.0): void
        {
            $this->addElement(new Slider($text, $min, $max, $step, $default));
        }

        /** @param string[] $steps */
        public function addStepSlider(string $text, array $steps, int $default = 0): void
        {
            $this->addElement(new StepSlider($text, $steps, $default));
        }

        public function addToggle(string $text, bool $default = false): void
        {
            $this->addElement(new Toggle($text, $default));
        }

        /** @return FormElement[] */
        public function getElements(): array
        {
            return $this->elements;
        }

        public function handleResponse(Player $player, $data): void
        {
            if ($data === null) {
                ($this->onSubmit)($player, null);
                return;
            }
            if (!is_array($data)) {
                throw new FormValidationException('Expected array response, got ' . get_debug_type($data));
            }
            if (count($data) !== count($this->elements)) {
                throw new FormValidationException(
                    'Expected ' . count($this->elements) . ' response entries, got ' . count($data)
                );
            }
            $answers = [];
            foreach ($this->elements as $index => $element) {
                $answers[$index] = $element->getValue($data[$index] ?? null);
            }
            ($this->onSubmit)($player, $answers);
        }

        public function jsonSerialize(): array
        {
            return [
                'type' => 'custom_form',
                'title' => $this->title,
                'content' => array_map(static fn(FormElement $element): array => $element->jsonSerialize(), $this->elements),
            ];
        }
    }

    /** `pocketmine\form\ModalForm` (`ext-forms`). */
    class ModalForm implements Form
    {
        /** @param \Closure(Player, ?bool): void $onSubmit */
        public function __construct(
            protected string $title,
            protected string $content,
            protected string $yesButtonText = 'gui.yes',
            protected string $noButtonText = 'gui.no',
            protected ?\Closure $onSubmit = null
        ) {}

        public function getTitle(): string
        {
            return $this->title;
        }

        public function getContent(): string
        {
            return $this->content;
        }

        public function handleResponse(Player $player, $data): void
        {
            if ($data === null) {
                if ($this->onSubmit !== null) {
                    ($this->onSubmit)($player, null);
                }
                return;
            }
            if (!is_bool($data)) {
                throw new FormValidationException('Expected bool response, got ' . get_debug_type($data));
            }
            if ($this->onSubmit !== null) {
                ($this->onSubmit)($player, $data);
            }
        }

        public function jsonSerialize(): array
        {
            return [
                'type' => 'modal',
                'title' => $this->title,
                'content' => $this->content,
                'button1' => $this->yesButtonText,
                'button2' => $this->noButtonText,
            ];
        }
    }
}

namespace pocketmine\runtime {
    use pocketmine\form\Form;

    /**
     * Pending forms per player, keyed by the id sent in the
     * `ModalFormRequestPacket`. `Player::sendForm()` stores the form here and
     * the runtime's `form_response` action takes it back before calling
     * `Form::handleResponse()`.
     *
     * This has no PM5 equivalent because upstream the form object lives in the
     * player's `PlayerSession`; the bridge has no native session object for
     * PHP, so a static registry is the smallest faithful substitute.
     */
    final class FormRegistry
    {
        private static int $nextId = 1;
        /** @var array<string, array<int, Form>> lowercase player => form id => form */
        private static array $pending = [];

        public static function nextId(): int
        {
            return self::$nextId++;
        }

        public static function store(string $player, int $id, Form $form): void
        {
            self::$pending[strtolower($player)][$id] = $form;
        }

        public static function take(string $player, int $id): ?Form
        {
            $key = strtolower($player);
            $form = self::$pending[$key][$id] ?? null;
            unset(self::$pending[$key][$id]);
            if (self::$pending[$key] === []) {
                unset(self::$pending[$key]);
            }
            return $form;
        }

        public static function clear(string $player): void
        {
            unset(self::$pending[strtolower($player)]);
        }
    }
}
