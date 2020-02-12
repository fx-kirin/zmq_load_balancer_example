import time
import zmq

context = zmq.Context()
socket = context.socket(zmq.REQ)
socket.connect("tcp://127.0.0.1:5672")

socket.send(b"Hello World")
message = socket.recv_multipart()
print("Result:%s" % (message))

socket.close()
context.destroy()
