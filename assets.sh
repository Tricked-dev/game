#!/bin/bash

function layerAse2png {
    aseprite -b --split-layers sprites/$1.aseprite --save-as public/assets/$1-{layer}.png
}

function ase2png {
    aseprite -b sprites/$1.aseprite --save-as public/assets/$1.png
}
ase2png dice-bg
ase2png number-base
ase2png start-again
ase2png background
ase2png deck-bg
ase2png start-btn
ase2png start-bg
ase2png waiting
layerAse2png dices
layerAse2png turns
layerAse2png ending
layerAse2png end-texts
