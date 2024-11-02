import tomli
import tomli_w
from pathlib import Path
import glob

def sync_version():
    # Read the version number of the workspace
    with open("pyproject.toml", "rb") as f:
        workspace_config = tomli.load(f)
        workspace_version = workspace_config["project"]["version"]

    # Update the version number of all lyric-* subprojects
    for project_path in glob.glob("lyric-*/pyproject.toml"):
        if "lyric-py/pyproject.toml" in project_path:
            continue
        with open(project_path, "rb") as f:
            project_config = tomli.load(f)

        if "version" not in project_config["project"]:
            continue

        # Update the version number
        project_config["project"]["version"] = workspace_version

        with open(project_path, "wb") as f:
            tomli_w.dump(project_config, f)

        print(f"Updated version in {project_path} to {workspace_version}")

if __name__ == "__main__":
    sync_version()