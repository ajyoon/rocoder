echo "Setting up systemd service"

sudo cp installation.service /lib/systemd/system/installation.service
sudo chmod 644 /lib/systemd/system/installation.service
sudo systemctl daemon-reload
sudo systemctl enable installation.service
sudo systemctl restart installation.service
sudo systemctl status installation.service
