import * as icons from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { NavItem } from "@/app/(dashboard)/_components/nav-main";
import navConfig from "./nav-items.json";

// Config-driven navigation: add entries to nav-items.json instead of editing
// this file. Each entry maps { title, url, icon } where `icon` is the name of
// a lucide-react icon (e.g. "Shield", "Hand", "LayoutDashboard").
//
// This avoids merge conflicts when multiple features add nav items — each
// feature only needs to add a single JSON line.

const iconMap = icons as unknown as Record<string, LucideIcon>;

export const navItems: NavItem[] = navConfig.map((item) => ({
  title: item.title,
  url: item.url,
  icon: iconMap[item.icon] ?? icons.Circle,
}));
