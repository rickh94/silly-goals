async function startRegistration() {
  let res = await fetch("/webauthn/register", {
    method: 'GET',
  });

  let creationOptions = await res.json();
  console.log(creationOptions);

  let attResp;
  try {
    attResp = await SimpleWebAuthnBrowser.startRegistration(creationOptions.publicKey);
  } catch (error) {
    // TODO: handle previously registered credentials elegantly
    alert("Something went wrong");
    console.debug(error);
    return;
  }


  const verificationResponse = await fetch("/webauthn/register", {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(attResp),
  });

  if (verificationResponse.ok) {
    alert("Registration successful");
  }
}

