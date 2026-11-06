<?php

declare(strict_types=1);

namespace BetterChat\command;

use pocketmine\command\CommandSender;
use pocketmine\player\Player;
use pocketmine\utils\TextFormat;
use BetterChat\Main;

class GroupCommand{

	public function __construct(private Main $plugin){}

	public function execute(CommandSender $sender, array $args) : bool{
		if(!$sender instanceof Player){
			$sender->sendMessage(TextFormat::colorize("&cThis command can only be used in-game."));
			return true;
		}

		$groupMgr = $this->plugin->getGroupManager();
		if(!$groupMgr->isEnabled()){
			$sender->sendMessage(TextFormat::colorize("&cChat groups are disabled."));
			return true;
		}

		if(empty($args)){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.command.usage")));
			return true;
		}

		$subCommand = strtolower(array_shift($args));

		return match($subCommand){
			"create" => $this->handleCreate($sender, $args),
			"delete" => $this->handleDelete($sender, $args),
			"invite" => $this->handleInvite($sender, $args),
			"accept" => $this->handleAccept($sender, $args),
			"deny" => $this->handleDeny($sender, $args),
			"leave" => $this->handleLeave($sender, $args),
			"kick" => $this->handleKick($sender, $args),
			"chat" => $this->handleChat($sender, $args),
			"list" => $this->handleList($sender),
			"info" => $this->handleInfo($sender, $args),
			default => (function() use($sender) : bool{
				$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.command.usage")));
				return true;
			})(),
		};
	}

	private function handleCreate(Player $sender, array $args) : bool{
		if(!$sender->hasPermission("betterchat.group.create")){
			$sender->sendMessage(TextFormat::colorize($this->gmsg("create.no-permission")));
			return true;
		}
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group create <name>"));
			return true;
		}

		$name = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();
		$official = isset($args[0]) && strtolower($args[0]) === "official" && $sender->hasPermission("betterchat.group.admin");

		$validationError = $groupMgr->validateName($name);
		if($validationError !== null){
			$sender->sendMessage($validationError);
			return true;
		}

		$error = $groupMgr->createGroup($name, $sender->getName(), $official);
		if($error !== null){
			$sender->sendMessage($error);
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace("{group}", $name, $this->plugin->msg("group.create.success"))
		));
		return true;
	}

	private function handleDelete(Player $sender, array $args) : bool{
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group delete <name>"));
			return true;
		}
		$name = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();
		$isAdmin = $sender->hasPermission("betterchat.group.admin");

		$error = $groupMgr->deleteGroup($name, $sender->getName(), $isAdmin);
		if($error !== null){
			$sender->sendMessage($error);
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace("{group}", $name, $this->plugin->msg("group.delete.success"))
		));
		return true;
	}

	private function handleInvite(Player $sender, array $args) : bool{
		if(count($args) < 2){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group invite <group> <player>"));
			return true;
		}
		$groupName = array_shift($args);
		$targetName = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();

		$target = $this->plugin->getServer()->getPlayerByPrefix($targetName);
		if($target === null){
			$sender->sendMessage(TextFormat::colorize(
				str_replace("{player}", $targetName, $this->plugin->msg("group.invite.target-offline"))
			));
			return true;
		}

		$error = $groupMgr->invitePlayer($groupName, $sender->getName(), $targetName);
		if($error !== null){
			$sender->sendMessage($error);
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace(["{player}", "{group}"], [$target->getDisplayName(), $groupName], $this->plugin->msg("group.invite.sent"))
		));

		// Notify target
		$notifyMsg = str_replace(
			["{sender}", "{group}"],
			[$sender->getDisplayName(), $groupName],
			$this->plugin->msg("group.invite.received")
		);
		$target->sendMessage(TextFormat::colorize($notifyMsg));
		return true;
	}

	private function handleAccept(Player $sender, array $args) : bool{
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group accept <group>"));
			return true;
		}
		$groupName = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();

		$error = $groupMgr->acceptInvite($sender->getName(), $groupName);
		if($error !== null){
			$sender->sendMessage($error);
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace("{group}", $groupName, $this->plugin->msg("group.accept.success"))
		));

		// Broadcast to group members
		$broadcast = $this->plugin->msg("group.accept.broadcast", "");
		if($broadcast !== ""){
			$msg = str_replace("{player}", $sender->getDisplayName(), $broadcast);
			foreach($groupMgr->getOnlineMembers($groupName) as $member){
				if($member !== $sender){
					$member->sendMessage(TextFormat::colorize($msg));
				}
			}
		}
		return true;
	}

	private function handleDeny(Player $sender, array $args) : bool{
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group deny <group>"));
			return true;
		}
		$groupName = array_shift($args);
		$this->plugin->getGroupManager()->denyInvite($sender->getName(), $groupName);
		$sender->sendMessage(TextFormat::colorize(
			str_replace("{group}", $groupName, $this->plugin->msg("group.deny.success"))
		));
		return true;
	}

	private function handleLeave(Player $sender, array $args) : bool{
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group leave <group>"));
			return true;
		}
		$groupName = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();

		$error = $groupMgr->leaveGroup($sender->getName(), $groupName);
		if($error !== null){
			$sender->sendMessage($error);
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace("{group}", $groupName, $this->plugin->msg("group.leave.success"))
		));
		return true;
	}

	private function handleKick(Player $sender, array $args) : bool{
		if(count($args) < 2){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group kick <group> <player>"));
			return true;
		}
		$groupName = array_shift($args);
		$targetName = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();
		$isAdmin = $sender->hasPermission("betterchat.group.admin");

		$error = $groupMgr->kickPlayer($groupName, $sender->getName(), $targetName, $isAdmin);
		if($error !== null){
			$sender->sendMessage($error);
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace(["{player}", "{group}"], [$targetName, $groupName], $this->plugin->msg("group.kick.success"))
		));

		// Notify kicked player
		$target = $this->plugin->getServer()->getPlayerByPrefix($targetName);
		if($target !== null){
			$target->sendMessage(TextFormat::colorize(
				str_replace("{group}", $groupName, $this->plugin->msg("group.kick.kicked-notification"))
			));
		}
		return true;
	}

	private function handleChat(Player $sender, array $args) : bool{
		$groupMgr = $this->plugin->getGroupManager();
		$current = $groupMgr->getActiveGroup($sender->getName());

		// /group chat [group] — toggle
		if(!empty($args)){
			$groupName = array_shift($args);
			$key = strtolower($groupName);
			if(!$groupMgr->groupExists($key)){
				$sender->sendMessage(TextFormat::colorize(
					str_replace("{group}", $groupName, $this->plugin->msg("group.chat.group-not-found"))
				));
				return true;
			}
			if(!$groupMgr->isMember($key, $sender->getName())){
				$sender->sendMessage(TextFormat::colorize(
					str_replace("{group}", $groupName, $this->plugin->msg("group.chat.not-member"))
				));
				return true;
			}
			if($current === $key){
				// Already in this group — toggle off
				$groupMgr->setActiveGroup($sender->getName(), null);
				$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.toggled-off")));
			}else{
				$groupMgr->setActiveGroup($sender->getName(), $key);
				$sender->sendMessage(TextFormat::colorize(
					str_replace("{group}", $groupName, $this->plugin->msg("group.toggled-on"))
				));
			}
			return true;
		}

		// /group chat (no args) — toggle current
		if($current !== null){
			$groupMgr->setActiveGroup($sender->getName(), null);
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.toggled-off")));
		}else{
			$groups = $groupMgr->getPlayerGroups($sender->getName());
			if(empty($groups)){
				$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.chat.not-in-group")));
				return true;
			}
			$first = $groups[0];
			$groupMgr->setActiveGroup($sender->getName(), $first);
			$sender->sendMessage(TextFormat::colorize(
				str_replace("{group}", $first, $this->plugin->msg("group.toggled-on"))
			));
		}
		return true;
	}

	private function handleList(Player $sender) : bool{
		$groupMgr = $this->plugin->getGroupManager();
		$groups = $groupMgr->getPlayerGroups($sender->getName());

		if(empty($groups)){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.list.none")));
			return true;
		}

		$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.list.header")));
		foreach($groups as $groupName){
			$group = $groupMgr->getGroup($groupName);
			$memberCount = $group !== null ? count($group["members"]) : 0;
			$msg = str_replace(
				["{group}", "{members}"],
				[$groupName, (string) $memberCount],
				$this->plugin->msg("group.list.entry")
			);
			$sender->sendMessage(TextFormat::colorize($msg));
		}
		return true;
	}

	private function handleInfo(Player $sender, array $args) : bool{
		if(count($args) < 1){
			$sender->sendMessage(TextFormat::colorize("&cUsage: /group info <group>"));
			return true;
		}
		$groupName = array_shift($args);
		$groupMgr = $this->plugin->getGroupManager();
		$group = $groupMgr->getGroup($groupName);

		if($group === null){
			$sender->sendMessage(TextFormat::colorize(
				str_replace("{group}", $groupName, $this->plugin->msg("group.info.not-found"))
			));
			return true;
		}

		$sender->sendMessage(TextFormat::colorize(
			str_replace("{group}", $groupName, $this->plugin->msg("group.info.header"))
		));
		$sender->sendMessage(TextFormat::colorize(
			str_replace("{owner}", $group["owner"], $this->plugin->msg("group.info.owner"))
		));
		$sender->sendMessage(TextFormat::colorize(
			str_replace("{members}", implode(", ", $group["members"]), $this->plugin->msg("group.info.members"))
		));
		if($group["official"]){
			$sender->sendMessage(TextFormat::colorize($this->plugin->msg("group.info.official")));
		}
		return true;
	}

	private function gmsg(string $key) : string{
		return $this->plugin->msg("group." . $key);
	}
}
