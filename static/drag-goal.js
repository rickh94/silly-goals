async function updateGoalStage(event, droppedOn) {
  const moving = document.getElementById(event.dataTransfer.getData('text/plain'));
  moving.remove();
  droppedOn.querySelector('.goal-list').prepend(moving);
  const newStage = droppedOn.dataset.stage;
  const oldStage = moving.dataset.stage;
  const goalId = moving.dataset.goalId;
  const groupId = moving.dataset.groupId;

  try {
    const res = await fetch(
      `/groups/${groupId}/goals/${goalId}/stage?stage=${newStage}`,
      {
        method: "PATCH",
      }
    )

    if (res.ok) {
      moving.dataset.stage = newStage;
      alert("updated");
    } else {
      moving.remove();
      const putBack = document.getElementById(`stage-${oldStage}`);
      putBack.querySelector('.goal-list').prepend(moving);
      alert("failed");
    }

  } catch (err) {
    alert("Something went wrong");
    console.log(err);
    moving.remove();
    const putBack = document.getElementById(`stage-${oldStage}`);
    putBack.querySelector('.goal-list').prepend(moving);
  }
}

function startDragging(event, dragging) {
  console.log(event);
  event.dataTransfer.effectAllowed = 'move';
  event.dataTransfer.setData('text/plain', dragging.id);
}
