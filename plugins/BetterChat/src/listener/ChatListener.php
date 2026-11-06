<?php

declare(strict_types=1);

namespace BetterChat\listener;

use pocketmine\event\Listener;
use pocketmine\event\player\PlayerChatEvent;
use pocketmine\event\player\PlayerJoinEvent;
use pocketmine\event\player\PlayerQuitEvent;
use pocketmine\event\server\CommandEvent;
use pocketmine\player\Player;
use pocketmine\utils\TextFormat;
use BetterChat\ChatFormatter;
use BetterChat\Main;

class ChatListener implements Listener{

	public function __construct(private Main $plugin){}

	/**
	 * @priority HIGH
	 */
	public function onChat(PlayerChatEvent $event) : void{
		$player = $event->getPlayer();
		$message = $event->getMessage();
		$config = $this->plugin->getConfig();

		// 1. Mute check
		if($this->plugin->getMuteManager()->isMuted($player)){
			$event->cancel();
			$remaining = $this->plugin->getMuteManager()->getRemainingTime($player->getName());
			$msg = $this->plugin->msg("mute.muted", "&cYou are muted. Time remaining: {time}");
			if($remaining > 0){
				$msg = str_replace("{time}", $this->formatDuration($remaining), $msg);
			}
			$player->sendMessage(TextFormat::colorize($msg));
			return;
		}

		// 2. Minigame answer check
		$mgMgr = $this->plugin->getMinigameManager();
		if($mgMgr->isEnabled() && $mgMgr->hasActiveGame()){
			$mgMgr->checkAnswer($player, $message);
		}

		// 3. Anti-spam (on raw message, before colorization)
		$spamMgr = $this->plugin->getSpamManager();
		if($spamMgr->isEnabled() && !$player->hasPermission($spamMgr->getBypassPermission())){
			$result = $spamMgr->check($player, $message);
			if($result !== null){
				$event->cancel();
				$player->sendMessage(TextFormat::colorize($result));
				return;
			}
		}

		// 4. Word filter (on raw message)
		$filterMgr = $this->plugin->getFilterManager();
		if($filterMgr->isEnabled() && !$player->hasPermission($filterMgr->getBypassPermission())){
			$filtered = $filterMgr->filter($message);
			if($filtered !== $message){
				$message = $filtered;
				$warning = $this->plugin->msg("filter.warning", "");
				if($warning !== ""){
					$player->sendMessage(TextFormat::colorize($warning));
				}
			}
		}

		// 4b. Anti-links
		$linkResult = $this->checkLinks($player, $message);
		if($linkResult !== null){
			$event->cancel();
			$player->sendMessage(TextFormat::colorize($linkResult));
			return;
		}

		// 5. Group chat routing
		$targetGroup = null;
		$groupMgr = $this->plugin->getGroupManager();
		if($groupMgr->isEnabled()){
			// Check for @groupname prefix
			if(preg_match('/^@(\S+)\s+(.+)$/s', $message, $matches)){
				$groupKey = strtolower($matches[1]);
				if($groupMgr->isMember($groupKey, $player->getName())){
					$targetGroup = $groupKey;
					$message = $matches[2];
				}
			}
			// Check toggle mode
			if($targetGroup === null){
				$active = $groupMgr->getActiveGroup($player->getName());
				if($active !== null){
					$targetGroup = $active;
				}
			}
			// Apply group routing
			if($targetGroup !== null){
				$onlineMembers = $groupMgr->getOnlineMembers($targetGroup);
				if(!empty($onlineMembers)){
					$event->setRecipients($onlineMembers);
				}
			}
		}

		// 6. Color code support (after spam/filter checks)
		if($config->getNested("color-codes.enabled", true)){
			$perm = $config->getNested("color-codes.permission", "betterchat.color");
			if($player->hasPermission($perm)){
				$message = TextFormat::colorize($message);
			}
		}

		// Update event message
		$event->setMessage($message);

		// 7. Set custom formatter (only if RankSystem is not handling chat)
		$useRankSystem = $config->getNested("ranksystem.enabled", true)
			&& $this->plugin->isRankSystemLoaded();

		if(!$useRankSystem){
			if($targetGroup !== null){
				$formatString = TextFormat::colorize($this->plugin->msg("group.chat-format", "&8[&b{group}&8] {display_name} > {message}"));
			}else{
				$formatString = TextFormat::colorize($this->plugin->msg("chat-format", "{display_name} > {message}"));
			}
			$prefix = $this->resolvePrefix($player);
			$suffix = $this->resolveSuffix($player);

			$placeholders = [
				"display_name" => $player->getDisplayName(),
				"prefix"       => TextFormat::colorize($prefix),
				"suffix"       => TextFormat::colorize($suffix),
				"world"        => $player->getWorld()->getFolderName(),
			];
			if($targetGroup !== null){
				$placeholders["group"] = $targetGroup;
			}

			$event->setFormatter(new ChatFormatter($formatString, $placeholders));
		}

		// 8. Mentions (skip for group messages)
		if($targetGroup === null && $config->getNested("mentions.enabled", true)){
			$this->handleMentions($player, $message);
		}
	}

	/**
	 * Handle command cooldowns via CommandEvent.
	 * @priority LOW
	 */
	public function onCommand(CommandEvent $event) : void{
		$sender = $event->getSender();
		if(!($sender instanceof Player)) return;

		$spamMgr = $this->plugin->getSpamManager();
		if(!$spamMgr->isCommandCooldownsEnabled()) return;
		if($sender->hasPermission($spamMgr->getCommandBypassPermission())) return;

		$fullCommand = $event->getCommand();
		$commandName = explode(" ", $fullCommand)[0];

		$result = $spamMgr->checkCommandCooldown($sender, $commandName);
		if($result !== null){
			$event->cancel();
			$sender->sendMessage(TextFormat::colorize($result));
		}
	}

	public function onQuit(PlayerQuitEvent $event) : void{
		$player = $event->getPlayer();
		$name = $player->getName();
		$this->plugin->getSpamManager()->clearPlayer($name);
		$this->plugin->getGroupManager()->clearPlayer($name);
		$this->plugin->getVoteMuteManager()->clearPlayer($name);

		$config = $this->plugin->getConfig();
		if($config->getNested("join-quit.enabled", true) && $config->getNested("join-quit.custom", true)){
			$msg = $this->plugin->msg("join-quit.quit", "&c- &7{player} &cleft the server");
			$msg = str_replace("{player}", $player->getDisplayName(), $msg);
			$event->setQuitMessage(TextFormat::colorize($msg));
		}
	}

	/**
	 * @priority NORMAL
	 */
	public function onJoin(PlayerJoinEvent $event) : void{
		$config = $this->plugin->getConfig();
		if(!$config->getNested("join-quit.enabled", true) || !$config->getNested("join-quit.custom", true)){
			return;
		}

		$player = $event->getPlayer();
		$hasPlayedBefore = $player->hasPlayedBefore();

		if(!$hasPlayedBefore){
			$msg = $this->plugin->msg("join-quit.first-join", "&a+ &7{player} &ajoined for the first time!");
		}else{
			$msg = $this->plugin->msg("join-quit.join", "&a+ &7{player} &ajoined the server");
		}

		$msg = str_replace("{player}", $player->getDisplayName(), $msg);
		$event->setJoinMessage(TextFormat::colorize($msg));
	}

	private function checkLinks(Player $player, string &$message) : ?string{
		$config = $this->plugin->getConfig();
		if(!$config->getNested("anti-links.enabled", true)) return null;
		if($player->hasPermission($config->getNested("anti-links.bypass-permission", "betterchat.bypass.links"))) return null;

		// URL pattern
		$pattern = '/(?:https?:\/\/|www\.)[^\s<>\[\]"\']+/i';
		if(!preg_match_all($pattern, $message, $matches)) return null;

		$whitelist = array_map('strtolower', $config->getNested("anti-links.whitelist", []));
		$hasBlocked = false;

		foreach($matches[0] as $url){
			$lower = strtolower($url);
			$allowed = false;
			foreach($whitelist as $domain){
				if(str_contains($lower, $domain)){
					$allowed = true;
					break;
				}
			}
			if(!$allowed) $hasBlocked = true;
		}

		if(!$hasBlocked) return null;

		$action = $config->getNested("anti-links.action", "filter");
		if($action === "block"){
			return $this->plugin->msg("anti-links.blocked", "&cLinks are not allowed in chat!");
		}

		// Filter: replace non-whitelisted URLs
		$replacement = $config->getNested("anti-links.replacement", "[LINK]");
		$message = preg_replace_callback($pattern, function(array $m) use ($whitelist, $replacement) : string{
			$lower = strtolower($m[0]);
			foreach($whitelist as $domain){
				if(str_contains($lower, $domain)) return $m[0];
			}
			return $replacement;
		}, $message);

		$warning = $this->plugin->msg("anti-links.filtered", "");
		return ($warning !== "") ? $warning : null;
	}

	private function resolvePrefix(Player $player) : string{
		// 1. Custom prefix (set via /betterchat setprefix)
		$custom = $this->plugin->getMuteManager()->getCustomPrefix($player);
		if($custom !== null){
			return $custom;
		}
		// 2. OP prefix
		if($player->hasOpPermission()){
			return $this->plugin->getConfig()->get("op-prefix", "");
		}
		// 3. Default
		return $this->plugin->getConfig()->get("default-prefix", "");
	}

	private function resolveSuffix(Player $player) : string{
		$custom = $this->plugin->getMuteManager()->getCustomSuffix($player);
		if($custom !== null){
			return $custom;
		}
		if($player->hasOpPermission()){
			return $this->plugin->getConfig()->get("op-suffix", "");
		}
		return $this->plugin->getConfig()->get("default-suffix", "");
	}

	private function handleMentions(Player $sender, string $message) : void{
		$config = $this->plugin->getConfig();
		$mentionFormat = $this->plugin->msg("mentions.format", "&e{sender} mentioned you: &f{message}");
		$cleanMessage = TextFormat::clean($message);

		foreach($this->plugin->getServer()->getOnlinePlayers() as $online){
			if($online === $sender) continue;
			if(!$online->hasPermission("betterchat.mention")) continue;
			$name = $online->getName();
			$pattern = '/\b' . preg_quote($name, '/') . '\b/i';
			if(preg_match($pattern, $cleanMessage)){
				$pingMsg = str_replace(
					["{sender}", "{message}"],
					[$sender->getDisplayName(), $message],
					$mentionFormat
				);
				$online->sendMessage(TextFormat::colorize($pingMsg));
			}
		}
	}

	private function formatDuration(int $seconds) : string{
		if($seconds >= 86400) return intdiv($seconds, 86400) . "d";
		if($seconds >= 3600) return intdiv($seconds, 3600) . "h";
		if($seconds >= 60) return intdiv($seconds, 60) . "m";
		return $seconds . "s";
	}
}
