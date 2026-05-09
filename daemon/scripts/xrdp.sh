#!/usr/bin/env bash

sudo apt install xfce4 xfce4-goodies -y
sudo apt install xrdp -y
sudo ufw allow 3389/tcp
sudo ufw reload
sudo adduser xrdp ssl-cert
echo "xfce4-session" > ~/.xsession
chmod +x ~/.xsession
sudo apt install flatpak -y
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install flathub org.mozilla.firefox
passwd

