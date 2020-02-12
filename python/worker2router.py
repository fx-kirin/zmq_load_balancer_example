import time
import zmq

context = zmq.Context()
socket = context.socket(zmq.REQ)
socket.connect("tcp://127.0.0.1:5673")

socket.send(b"READY")
index = 0
while True:
    index += 1
    print("Message was sent")
    message = socket.recv_multipart()
    print("Result:%s" % (message))
    result = 'Worker Result:%s' % (index)
    socket.send_multipart([message[0], bytes(result, 'utf-8')])
    time.sleep(1)
