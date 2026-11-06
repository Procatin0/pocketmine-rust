<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\command\CommandSender;
use pocketmine\console\ConsoleCommandSender;
use pocketmine\player\Player;
use pocketmine\plugin\PluginBase;
use pocketmine\scheduler\ClosureTask;
use pocketmine\scheduler\TaskHandler;
use pocketmine\utils\Config;
use pocketmine\utils\TextFormat;

class MinigameManager{

	private bool $enabled;
	private int $autoStartInterval;
	private int $timeLimit;
	private string $startMessage;
	private string $winMessage;
	private string $expireMessage;
	private string $noQuestionsMessage;
	private string $alreadyRunningMessage;
	private string $stoppedMessage;
	/** @var string[] */
	private array $rewardCommands;

	private bool $triviaEnabled;
	/** @var array<int, array{question: string, answer: string}> */
	private array $triviaQuestions;
	private bool $unscrambleEnabled;
	/** @var string[] */
	private array $unscrambleWords;
	private bool $mathEnabled;
	private int $mathMinNumber;
	private int $mathMaxNumber;
	/** @var string[] */
	private array $mathOperations;

	/** @var \stdClass|null {type: string, prompt: string, answer: string} */
	private ?\stdClass $activeGame = null;
	private ?TaskHandler $timerHandler = null;
	private ?TaskHandler $autoStartHandler = null;

	public function __construct(
		private PluginBase $plugin,
		Config $config,
		Config $messages
	){
		$p = "minigames.";
		$this->enabled = $config->getNested($p . "enabled", true);
		$this->autoStartInterval = $config->getNested($p . "auto-start-interval", 300);
		$this->timeLimit = $config->getNested($p . "time-limit", 30);
		$this->startMessage = $messages->getNested("minigame.start", "&e[Minigame] &a{type}: &f{question}");
		$this->winMessage = $messages->getNested("minigame.win", "&e[Minigame] &6{winner} &awon!");
		$this->expireMessage = $messages->getNested("minigame.expire", "&e[Minigame] &cTime's up!");
		$this->noQuestionsMessage = $messages->getNested("minigame.no-questions", "&cNo questions available.");
		$this->alreadyRunningMessage = $messages->getNested("minigame.already-running", "&cA minigame is already running!");
		$this->stoppedMessage = $messages->getNested("minigame.stopped", "&e[Minigame] &7Minigame stopped.");
		$this->rewardCommands = $config->getNested($p . "reward-commands", []);

		$this->triviaEnabled = $config->getNested($p . "trivia.enabled", true);
		$this->unscrambleEnabled = $config->getNested($p . "unscramble.enabled", true);
		$this->mathEnabled = $config->getNested($p . "math.enabled", true);
		$this->mathMinNumber = $config->getNested($p . "math.min-number", 1);
		$this->mathMaxNumber = $config->getNested($p . "math.max-number", 50);
		$this->mathOperations = $config->getNested($p . "math.operations", ["+", "-"]);

		$this->loadGameData();

		if($this->enabled && $this->autoStartInterval > 0){
			$this->scheduleAutoStart();
		}
	}

	public function isEnabled() : bool{
		return $this->enabled;
	}

	public function hasActiveGame() : bool{
		return $this->activeGame !== null;
	}

	public function getEnabledTypes() : array{
		$types = [];
		if($this->triviaEnabled) $types[] = "trivia";
		if($this->unscrambleEnabled) $types[] = "unscramble";
		if($this->mathEnabled) $types[] = "math";
		return $types;
	}

	public function getNoQuestionsMessage() : string{ return $this->noQuestionsMessage; }
	public function getAlreadyRunningMessage() : string{ return $this->alreadyRunningMessage; }
	public function getStoppedMessage() : string{ return $this->stoppedMessage; }

	private function loadGameData() : void{
		$dataFolder = $this->plugin->getDataFolder();
		$dataFile = $dataFolder . "minigames.yml";

		if(!file_exists($dataFile)){
			$this->plugin->saveResource("minigames.yml");
		}

		$data = new Config($dataFile, Config::YAML);
		$this->triviaQuestions = $data->get("trivia", []);
		$this->unscrambleWords = $data->get("unscramble", []);
	}

	public function startRandomGame() : void{
		if($this->hasActiveGame()) return;
		$types = $this->getEnabledTypes();
		if(empty($types)) return;
		$type = $types[array_rand($types)];
		$this->startGame($type);
	}

	public function startGame(string $type) : bool{
		if($this->hasActiveGame()) return false;

		$prompt = "";
		$answer = "";

		switch($type){
			case "trivia":
				if(empty($this->triviaQuestions)) return false;
				$entry = $this->triviaQuestions[array_rand($this->triviaQuestions)];
				$prompt = (string) ($entry["question"] ?? "");
				$answer = strtolower((string) ($entry["answer"] ?? ""));
				break;
			case "unscramble":
				if(empty($this->unscrambleWords)) return false;
				$word = (string) $this->unscrambleWords[array_rand($this->unscrambleWords)];
				$answer = strtolower($word);
				$prompt = $this->shuffleString($word);
				break;
			case "math":
				[$prompt, $answer] = $this->generateMathProblem();
				break;
			default:
				return false;
		}

		$game = new \stdClass();
		$game->type = $type;
		$game->prompt = $prompt;
		$game->answer = $answer;
		$this->activeGame = $game;

		$msg = str_replace(
			["{type}", "{question}"],
			[ucfirst($type), $prompt],
			$this->startMessage
		);
		$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($msg));

		$this->timerHandler = $this->plugin->getScheduler()->scheduleDelayedTask(
			new ClosureTask(function() : void{
				$this->expireGame();
			}),
			$this->timeLimit * 20
		);

		return true;
	}

	public function stopGame() : void{
		if($this->timerHandler !== null){
			$this->timerHandler->cancel();
			$this->timerHandler = null;
		}
		$this->activeGame = null;
	}

	public function shutdown() : void{
		$this->stopGame();
		if($this->autoStartHandler !== null){
			$this->autoStartHandler->cancel();
			$this->autoStartHandler = null;
		}
	}

	public function checkAnswer(Player $player, string $message) : bool{
		if(!$this->hasActiveGame()) return false;
		if(strtolower(trim($message)) === $this->activeGame->answer){
			$this->processWin($player);
			return true;
		}
		return false;
	}

	private function processWin(Player $player) : void{
		if($this->timerHandler !== null){
			$this->timerHandler->cancel();
			$this->timerHandler = null;
		}

		$msg = str_replace(
			["{winner}", "{answer}"],
			[$player->getDisplayName(), $this->activeGame->answer],
			$this->winMessage
		);
		$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($msg));

		$server = $this->plugin->getServer();
		$console = new ConsoleCommandSender($server, $server->getLanguage());
		foreach($this->rewardCommands as $cmd){
			$execCmd = str_replace("{winner}", $player->getName(), (string) $cmd);
			$server->dispatchCommand($console, $execCmd);
		}

		$this->activeGame = null;
	}

	private function expireGame() : void{
		if(!$this->hasActiveGame()) return;
		$msg = str_replace(
			"{answer}",
			$this->activeGame->answer,
			$this->expireMessage
		);
		$this->plugin->getServer()->broadcastMessage(TextFormat::colorize($msg));
		$this->activeGame = null;
		$this->timerHandler = null;
	}

	private function scheduleAutoStart() : void{
		$this->autoStartHandler = $this->plugin->getScheduler()->scheduleRepeatingTask(
			new ClosureTask(function() : void{
				$this->startRandomGame();
			}),
			$this->autoStartInterval * 20
		);
	}

	private function shuffleString(string $word) : string{
		$chars = str_split($word);
		shuffle($chars);
		return implode("", $chars);
	}

	/** @return array{string, string} [$question, $answer] */
	private function generateMathProblem() : array{
		$op = $this->mathOperations[array_rand($this->mathOperations)];
		$a = random_int($this->mathMinNumber, $this->mathMaxNumber);
		$b = random_int($this->mathMinNumber, $this->mathMaxNumber);

		switch($op){
			case "-":
				if($a < $b){ $tmp = $a; $a = $b; $b = $tmp; }
				$answer = $a - $b;
				break;
			case "*":
				$answer = $a * $b;
				break;
			default:
				$answer = $a + $b;
				$op = "+";
		}

		return ["{$a} {$op} {$b}", (string) $answer];
	}
}
