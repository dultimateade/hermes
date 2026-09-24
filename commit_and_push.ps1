# Git commit and push script

cd 'c:\Users\LENOVO PC\Desktop\New folder (7)\hermes'

# Stage all changes
git add .

# Create commit
git commit -F '.\.git_commit_msg.txt'

# Push to origin
git push -u origin feat/canonical-error-registry

# Display status
git log --oneline -3
