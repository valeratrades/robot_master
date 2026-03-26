from z_variablesDeTest import new_plateau_test, new_small_plateau_test, small_plateau_test
from c_joueuses import init_tuple_joueuses, configuration_textuel, choix_carte_random, choix_carte_manuel, choix_et_pose_carte



# Test init_tuple_joueuses
def test_init_tuple_joueuses_1(monkeypatch):
	responses = iter(["A","B"])
	monkeypatch.setattr('builtins.input', lambda msg: next(responses))
	assert init_tuple_joueuses()==("A","B")

def test_init_tuple_joueuses_2():
	assert init_tuple_joueuses({'v':False})==("Alice","Bob")

# Test configuration_textuel
def test_configuration_textuel(monkeypatch):
	responses = iter(["r","m"])
	monkeypatch.setattr('builtins.input', lambda msg: next(responses))
	assert configuration_textuel(("A","B"))=={0 : ["A","r",{}], 1 : ["B","m",{}]}


########## Test choix / pose carte
p = [[0,None],[2,3]]
dico_main={0:0,1:0,2:0,3:2,4:0,5:0}
option1={'maxC':5}

# Test random une seule solution possible
def test_choix_carte_random ():
	assert choix_carte_random(p,dico_main,"plop",option1)==(3,0,1)

# Test manuel input correct
def test_choix_carte_manuel_1_bienveillant(monkeypatch):
	responses = iter(["3","0","1"])
	monkeypatch.setattr('builtins.input', lambda msg: next(responses))
	assert choix_carte_manuel(p,dico_main,"plop",option1)==(3,0,1)

p2 = [[0,1],[2,None]]
dico_main2={0:0,1:1,2:0,3:0,4:0,5:0}
option2={'maxC':5}

# Test manuel input incorrect at first
def test_choix_carte_manuel_2_mauvaise_entree(monkeypatch):
	responses = iter(["5","3","1","9","1","1","1","1","1","1"])
	monkeypatch.setattr('builtins.input', lambda msg: next(responses))
	assert choix_carte_manuel(p2,dico_main2,"plop",option2)==(1,1,1)

# Test manuel input incorrect at first
def test_choix_carte_manuel_3_utilisation_try(monkeypatch):
	responses = iter(["az","3","1","9","bx","1","1","1","1","1","1","1","1"])
	monkeypatch.setattr('builtins.input', lambda msg: next(responses))
	assert choix_carte_manuel(p2,dico_main2,"plop",option2)==(1,1,1)



# test_choix_et_pose_carte joueuse random
def test_test_choix_et_pose_carte():
	p = [[0,1],[2,None]]
	dico_main={0:0,1:1,2:0,3:0,4:0,5:0}
	dico_joueuses={0:["A","r",dico_main],1:["B","r",{3:1}]}
	option={'v':False,'maxC':5}
	choix_et_pose_carte(p,dico_joueuses,option,0)
	assert p==[[0,1],[2,1]] and dico_main=={0:0,1:0,2:0,3:0,4:0,5:0}

# test_choix_et_pose_carte joueuse manuel
def test_test_choix_et_pose_carte_2(monkeypatch):
	responses = iter(["3","1","1"])
	monkeypatch.setattr('builtins.input', lambda msg: next(responses))
	p = [[0,1],[2,None]]
	dico_main={0:0,1:1,2:0,3:0,4:0,5:0}
	dico_joueuses={0:["A","r",dico_main],1:["B","m",{3:1}]}
	option={'v':False,'maxC':5}
	choix_et_pose_carte(p,dico_joueuses,option,1)
	assert p==[[0,1],[2,3]] and dico_joueuses=={0:["A","r",dico_main],1:["B","m",{3:0}]}
