<?php

declare(strict_types=1);

namespace BetterChat\manager;

use pocketmine\player\Player;
use pocketmine\plugin\PluginBase;
use pocketmine\utils\Config;
use pocketmine\utils\TextFormat;

class GroupManager{

	private bool $enabled;
	private int $maxGroupsPerPlayer;
	private int $maxMembersPerGroup;
	private int $inviteTimeout;
	private int $minNameLength;
	private int $maxNameLength;
	private string $namePattern;

	/** @var array<string, array{owner: string, members: string[], official: bool, created: int}> */
	private array $groups = [];
	/** @var array<string, string> */
	private array $activeGroup = [];
	/** @var array<string, int> */
	private array $pendingInvites = [];

	public function __construct(
		private PluginBase $plugin,
		Config $config,
		private Config $messages
	){
		$p = "groups.";
		$this->enabled = $config->getNested($p . "enabled", true);
		$this->maxGroupsPerPlayer = $config->getNested($p . "max-groups-per-player", 3);
		$this->maxMembersPerGroup = $config->getNested($p . "max-members-per-group", 20);
		$this->inviteTimeout = $config->getNested($p . "invite-timeout", 60);
		$this->minNameLength = $config->getNested($p . "min-name-length", 3);
		$this->maxNameLength = $config->getNested($p . "max-name-length", 16);
		$this->namePattern = $config->getNested($p . "name-pattern", "^[a-zA-Z0-9_-]+$");

		$this->load();
	}

	public function isEnabled() : bool{
		return $this->enabled;
	}

	// --- Group CRUD ---

	public function createGroup(string $name, string $ownerName, bool $official = false) : string|null{
		$key = strtolower($name);
		if(isset($this->groups[$key])){
			return $this->msg("create.name-taken", ["{group}" => $name]);
		}
		if(!$official && $this->countOwnedGroups($ownerName) >= $this->maxGroupsPerPlayer){
			return $this->msg("create.limit-reached", ["{max}" => (string) $this->maxGroupsPerPlayer]);
		}
		$this->groups[$key] = [
			"owner" => strtolower($ownerName),
			"members" => [strtolower($ownerName)],
			"official" => $official,
			"created" => time(),
		];
		$this->save();
		return null;
	}

	public function deleteGroup(string $name, string $deleterName, bool $isAdmin = false) : string|null{
		$key = strtolower($name);
		if(!isset($this->groups[$key])){
			return $this->msg("delete.not-found", ["{group}" => $name]);
		}
		if(!$isAdmin && $this->groups[$key]["owner"] !== strtolower($deleterName)){
			return $this->msg("delete.not-owner");
		}
		// Clear active chat for all members
		foreach($this->groups[$key]["members"] as $member){
			if(($this->activeGroup[strtolower($member)] ?? null) === $key){
				unset($this->activeGroup[strtolower($member)]);
			}
		}
		unset($this->groups[$key]);
		$this->save();
		return null;
	}

	public function groupExists(string $name) : bool{
		return isset($this->groups[strtolower($name)]);
	}

	public function getGroup(string $name) : ?array{
		return $this->groups[strtolower($name)] ?? null;
	}

	// --- Membership ---

	public function isMember(string $groupName, string $playerName) : bool{
		$key = strtolower($groupName);
		return isset($this->groups[$key]) && in_array(strtolower($playerName), $this->groups[$key]["members"], true);
	}

	public function isOwner(string $groupName, string $playerName) : bool{
		$key = strtolower($groupName);
		return isset($this->groups[$key]) && $this->groups[$key]["owner"] === strtolower($playerName);
	}

	public function invitePlayer(string $groupName, string $inviterName, string $invitedName) : string|null{
		$key = strtolower($groupName);
		if(!isset($this->groups[$key])){
			return $this->msg("delete.not-found", ["{group}" => $groupName]);
		}
		if($this->groups[$key]["owner"] !== strtolower($inviterName)){
			return $this->msg("invite.not-owner");
		}
		if($this->isMember($groupName, $invitedName)){
			return $this->msg("invite.already-member", ["{player}" => $invitedName]);
		}
		if(count($this->groups[$key]["members"]) >= $this->maxMembersPerGroup){
			return $this->msg("invite.group-full", ["{max}" => (string) $this->maxMembersPerGroup]);
		}
		$inviteKey = strtolower($invitedName) . ":" . $key;
		$this->pendingInvites[$inviteKey] = time();
		return null;
	}

	public function hasPendingInvite(string $playerName, string $groupName) : bool{
		$inviteKey = strtolower($playerName) . ":" . strtolower($groupName);
		if(!isset($this->pendingInvites[$inviteKey])){
			return false;
		}
		if(time() - $this->pendingInvites[$inviteKey] > $this->inviteTimeout){
			unset($this->pendingInvites[$inviteKey]);
			return false;
		}
		return true;
	}

	public function acceptInvite(string $playerName, string $groupName) : string|null{
		$key = strtolower($groupName);
		$inviteKey = strtolower($playerName) . ":" . $key;

		if(!isset($this->pendingInvites[$inviteKey])){
			return $this->msg("accept.no-pending", ["{group}" => $groupName]);
		}
		if(time() - $this->pendingInvites[$inviteKey] > $this->inviteTimeout){
			unset($this->pendingInvites[$inviteKey]);
			return $this->msg("invite.expired");
		}

		unset($this->pendingInvites[$inviteKey]);
		$this->groups[$key]["members"][] = strtolower($playerName);
		$this->save();
		return null;
	}

	public function denyInvite(string $playerName, string $groupName) : void{
		$inviteKey = strtolower($playerName) . ":" . strtolower($groupName);
		unset($this->pendingInvites[$inviteKey]);
	}

	public function leaveGroup(string $playerName, string $groupName) : string|null{
		$key = strtolower($groupName);
		if(!isset($this->groups[$key])){
			return $this->msg("delete.not-found", ["{group}" => $groupName]);
		}
		$lname = strtolower($playerName);
		if(!in_array($lname, $this->groups[$key]["members"], true)){
			return $this->msg("leave.not-member");
		}
		if($this->groups[$key]["owner"] === $lname){
			return $this->msg("leave.owner-cannot-leave");
		}
		$this->groups[$key]["members"] = array_values(array_diff($this->groups[$key]["members"], [$lname]));
		if(($this->activeGroup[$lname] ?? null) === $key){
			unset($this->activeGroup[$lname]);
		}
		$this->save();
		return null;
	}

	public function kickPlayer(string $groupName, string $kickerName, string $targetName, bool $isAdmin = false) : string|null{
		$key = strtolower($groupName);
		if(!isset($this->groups[$key])){
			return $this->msg("delete.not-found", ["{group}" => $groupName]);
		}
		if(!$isAdmin && $this->groups[$key]["owner"] !== strtolower($kickerName)){
			return $this->msg("kick.not-owner");
		}
		$ltarget = strtolower($targetName);
		if(!in_array($ltarget, $this->groups[$key]["members"], true)){
			return $this->msg("kick.not-member", ["{player}" => $targetName]);
		}
		$this->groups[$key]["members"] = array_values(array_diff($this->groups[$key]["members"], [$ltarget]));
		if(($this->activeGroup[$ltarget] ?? null) === $key){
			unset($this->activeGroup[$ltarget]);
		}
		$this->save();
		return null;
	}

	// --- Active chat ---

	public function setActiveGroup(string $playerName, ?string $groupName) : void{
		$lname = strtolower($playerName);
		if($groupName === null){
			unset($this->activeGroup[$lname]);
		}else{
			$this->activeGroup[$lname] = strtolower($groupName);
		}
	}

	public function getActiveGroup(string $playerName) : ?string{
		$lname = strtolower($playerName);
		$active = $this->activeGroup[$lname] ?? null;
		if($active !== null && !isset($this->groups[$active])){
			unset($this->activeGroup[$lname]);
			return null;
		}
		return $active;
	}

	// --- Queries ---

	/** @return string[] */
	public function getPlayerGroups(string $playerName) : array{
		$lname = strtolower($playerName);
		$result = [];
		foreach($this->groups as $key => $group){
			if(in_array($lname, $group["members"], true)){
				$result[] = $key;
			}
		}
		return $result;
	}

	/** @return Player[] */
	public function getOnlineMembers(string $groupName) : array{
		$key = strtolower($groupName);
		if(!isset($this->groups[$key])){
			return [];
		}
		$online = [];
		$server = $this->plugin->getServer();
		foreach($this->groups[$key]["members"] as $memberName){
			$player = $server->getPlayerByPrefix($memberName);
			if($player !== null){
				$online[] = $player;
			}
		}
		return $online;
	}

	/** @return string[] */
	public function getPendingInvites(string $playerName) : array{
		$prefix = strtolower($playerName) . ":";
		$result = [];
		$now = time();
		foreach($this->pendingInvites as $key => $timestamp){
			if(str_starts_with($key, $prefix)){
				if($now - $timestamp > $this->inviteTimeout){
					unset($this->pendingInvites[$key]);
				}else{
					$result[] = substr($key, strlen($prefix));
				}
			}
		}
		return $result;
	}

	// --- Validation ---

	public function validateName(string $name) : ?string{
		$len = strlen($name);
		if($len < $this->minNameLength || $len > $this->maxNameLength){
			return $this->msg("create.invalid-name", [
				"{min}" => (string) $this->minNameLength,
				"{max}" => (string) $this->maxNameLength,
			]);
		}
		if(!preg_match('/' . $this->namePattern . '/', $name)){
			return $this->msg("create.invalid-name", [
				"{min}" => (string) $this->minNameLength,
				"{max}" => (string) $this->maxNameLength,
			]);
		}
		return null;
	}

	public function getMaxMembers() : int{
		return $this->maxMembersPerGroup;
	}

	// --- Lifecycle ---

	public function clearPlayer(string $playerName) : void{
		unset($this->activeGroup[strtolower($playerName)]);
	}

	public function save() : void{
		$filePath = $this->plugin->getDataFolder() . "groups.yml";
		$config = new Config($filePath, Config::YAML);
		$config->setAll(["groups" => $this->groups]);
		$config->save();
	}

	public function load() : void{
		$filePath = $this->plugin->getDataFolder() . "groups.yml";
		if(!file_exists($filePath)){
			return;
		}
		$config = new Config($filePath, Config::YAML);
		$loaded = $config->get("groups", []);
		$this->groups = [];
		foreach($loaded as $key => $group){
			$key = strtolower($key);
			if(!is_array($group)) continue;
			$members = $group["members"] ?? [];
			if(empty($members)) continue;
			$this->groups[$key] = [
				"owner" => strtolower((string) ($group["owner"] ?? $members[0])),
				"members" => array_map("strtolower", $members),
				"official" => (bool) ($group["official"] ?? false),
				"created" => (int) ($group["created"] ?? time()),
			];
			// Ensure owner is in members
			if(!in_array($this->groups[$key]["owner"], $this->groups[$key]["members"], true)){
				$this->groups[$key]["members"][] = $this->groups[$key]["owner"];
			}
		}
	}

	// --- Helpers ---

	private function countOwnedGroups(string $playerName) : int{
		$lname = strtolower($playerName);
		$count = 0;
		foreach($this->groups as $group){
			if($group["owner"] === $lname) $count++;
		}
		return $count;
	}

	private function msg(string $key, array $replacements = []) : string{
		$msg = $this->messages->getNested("group." . $key, "");
		foreach($replacements as $search => $replace){
			$msg = str_replace($search, $replace, $msg);
		}
		return TextFormat::colorize($msg);
	}
}
