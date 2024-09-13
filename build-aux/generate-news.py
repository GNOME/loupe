#!/usr/bin/env python3

import os
import os.path
import sys

release_names = []

BASE_DIR = 'news.d'
OUT_FILE = 'NEWS'
HEADING = ''

def main():
    changelog = Changelog(BASE_DIR, HEADING)
    changelog.load()

    with open(OUT_FILE, 'w') as f:
        f.write(changelog.format())

class Release:
    def __init__(self, name):
        self.name = name

        self.released = "Unreleased"
        self.security = []
        self.added = []
        self.fixed = []
        self.changed = []
        self.removed = []
        self.deprecated = []

    def add(self, entry: os.DirEntry):
        with open(entry) as f:
            content = f.read().strip()

        if entry.name == "released":
            self.released = content
            return

        entry_type = entry.name.split('-')[0]

        match entry_type:
            case 'security':
                store = self.security
            case 'added':
                store = self.added
            case 'fixed':
                store = self.fixed
            case 'changed':
                store = self.changed
            case 'removed':
                store = self.removed
            case 'deprecated':
                store = self.deprecated
            case _:
                print(f'WARNING: Unknown entry type "{entry_type}" in "{self.name}"', file=sys.stderr)
                return;

        with open(entry) as f:
            store.append(content)

    def sections(self):
        categories = []

        if self.security:
            categories.append(("Security", self.security))
        if self.added:
            categories.append(("Added", self.added))
        if self.fixed:
            categories.append(("Fixed", self.fixed))
        if self.changed:
            categories.append(("Changed", self.changed))
        if self.removed:
            categories.append(("Removed", self.removed))
        if self.deprecated:
            categories.append(("Deprecated", self.deprecated))

        return categories

    def format(self):
        heading =f"{self.name} ({self.released})"

        s = f"## {heading}\n"

        for (section, items) in self.sections():
            s += f"\n### {section}\n\n"

            for item in sorted(items):
                s += f"- {item}\n"

        return s

    def __lt__(self, other):
         if self.released != other.released:
            return self.released < other.released

         return self.name < other.name

class Changelog:
    def __init__(self, path, heading):
        self.path = path
        self.heading = heading
        self.previous = ""
        self.releases = []

    def load(self):
        with os.scandir(BASE_DIR) as it:
            for entry in it:
                if entry.is_dir():
                    release_names.append(entry.name)
                elif entry.is_file() and entry.name == "previous":
                    with open(entry.path) as f:
                        self.previous = f.read()

        for release_name in release_names:
            release = Release(release_name)
            self.add(release)
            with os.scandir(os.path.join(BASE_DIR, release_name)) as it:
                for entry in it:
                    if entry.is_file():
                        release.add(entry)

    def add(self, release: Release):
        self.releases.append(release)

    def format(self):
        s = ""

        if self.heading:
            s += f'# {self.heading}\n'

        for release in reversed(sorted(self.releases)):
            if s != "":
                s += "\n"
            s += release.format()

        if self.previous:
            s += '\n'
            s += self.previous


        return s

main()
