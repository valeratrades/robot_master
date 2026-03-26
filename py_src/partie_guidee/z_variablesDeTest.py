from collections import Counter #pour comparer peu importe l'ordre
import pytest


def new_plateau_test():
	return [[None,None,1,1,0],[None,2,None,3,None],[4,None,None,None,None],[None,2,None,None,0],[4,4,4,0,0]]
plateau_test=new_plateau_test()

def new_small_plateau_test():
	return ([[None,1,2],[3,None,None],[4,None,None]])

small_plateau_test= new_small_plateau_test()

# Pour afficher les plateaux de test:
# from a_plateau import *
# print (plateau_to_string(plateau_test))
# print (plateau_to_string(small_plateau_test))
